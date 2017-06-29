// Copyright (C) 2017  ParadoxSpiral
//
// This file is part of parabot.
//
// Parabot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Parabot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Parabot.  If not, see <http://www.gnu.org/licenses/>.

use diesel;
use diesel::prelude::*;
use forecast::{ApiResponse, ApiClient, ForecastRequestBuilder, ExcludeBlock, ExtendBy, Units};
use irc::client::prelude::*;
use parking_lot::RwLock;
use regex::Regex;
use reqwest::Client;
use serde_json::de;
use serde_json::Value;
use slog::Logger;

use std::collections::HashMap;
use std::io::Read;

use config::{ChannelCfg, Config, ServerCfg};
use models;
use schema;
use schema::last_weather_search::dsl;

const GEOCODING_API_BASE: &str = "http://www.mapquestapi.com/geocoding/v1/address";

lazy_static!{
    static ref LAST_WEATHER_CACHE: RwLock<HashMap<(String, String), String>> = {
        RwLock::new(HashMap::new())
    };
}

// Read DB to get init values
pub fn init(cfg: &Config, log: &Logger) {
    let mut cache = LAST_WEATHER_CACHE.write();
    for srv in &cfg.servers {
        let conn = super::establish_database_connection(srv, log);
        let queries = dsl::last_weather_search
            .filter(dsl::server.eq(&srv.address))
            .load::<models::LastWeatherSearch>(&conn);
        if queries.is_err() {
            crit!(
                log,
                "Failed to load last weather searches: {:?}",
                queries.as_ref().unwrap_err()
            );
            panic!(
                "Failed to load last weather searches: {:?}",
                queries.unwrap_err()
            )
        } else {
            debug!(
                log,
                "Last weather searches: {:?}",
                queries.as_ref().unwrap()
            );
            for q in queries.unwrap() {
                cache.insert((srv.address.clone(), q.nick.clone()), q.location);
            }
            cache.shrink_to_fit();
        };
    }
}

pub fn handle(
    cfg: &ServerCfg,
    chan_cfg: Option<&ChannelCfg>,
    srv: &IrcServer,
    log: &Logger,
    msg: &str,
    nick: &str,
) -> String {
    let (future, n, hours, days, location) = {
        // Use last location
        if msg.is_empty() {
            (false, None, false, false, {
                if let Some(loc) = LAST_WEATHER_CACHE
                    .read()
                    .get(&(cfg.address.clone(), nick.to_owned()))
                {
                    loc.clone()
                } else {
                    return "You have never used `.weather` before, try `.help weather`".into();
                }
            })
        } else {
            // Only compile the regex once
            lazy_static! {
		        static ref REGEX: Regex = Regex::new("\
		        	(?P<plus>\\+){0,1}\
		        	(?P<digits>\\d+){0,1}\
		        	(?:(?P<h>h)|\
		        	    (?P<d>d)|\
		        	    (?:\
		        	   	    \\s{0,}\
			        	    (?: (?P<hours>hours)|\
			        	    	(?P<days>days)))\
		        	){0,1}\
		        	\\s{0,}\
		        	(?P<location>.+)").unwrap();
		    }

            let captures = if let Some(caps) = REGEX.captures(msg) {
                trace!(log, "Weather captures: {:?}", caps);
                caps
            } else {
                debug!(log, "No captures");
                return "Invalid `.weather` syntax, try: `.help weather`".into();
            };

            (
                captures.name("plus").is_some(),
                captures
                    .name("digits")
                    .and_then(|m| Some(m.as_str().to_owned())),
                captures.name("h").is_some() || captures.name("hours").is_some(),
                captures.name("d").is_some() || captures.name("days").is_some(),
                if let Some(loc) = captures.name("location") {
                    let new_loc = loc.as_str().to_owned();
                    // Potentially update the cache and DB
                    let mut cache = LAST_WEATHER_CACHE.write();
                    if let Some(old_loc) = cache
                        .get(&(cfg.address.clone(), nick.to_owned()))
                        .cloned()
                    {
                        // Only update if the location actually changed
                        if old_loc != new_loc {
                            trace!(log, "Updating Cache/DB");
                            cache.remove(&(cfg.address.clone(), nick.to_owned()));
                            cache.insert((cfg.address.clone(), nick.to_owned()), new_loc.clone());
                            drop(cache);

                            let conn = super::establish_database_connection(cfg, log);
                            if let Err(e) = diesel::update(
                                dsl::last_weather_search
                                    .filter(dsl::server.eq(&cfg.address))
                                    .filter(dsl::nick.eq(nick)),
                            ).set(dsl::location.eq(new_loc.clone()))
                                .execute(&conn)
                            {
                                crit!(log, "Failed to update weather location: {:?}", e);
                            }
                        } else {
                            trace!(log, "No update needed")
                        }
                    } else {
                        trace!(log, "Inserting into Cache/DB");
                        cache.insert((cfg.address.clone(), nick.to_owned()), new_loc.clone());
                        drop(cache);

                        let conn = super::establish_database_connection(cfg, log);
                        let new = models::NewLastWeatherSearch {
                            server: &cfg.address,
                            nick: nick,
                            location: &*new_loc,
                        };
                        if let Err(e) = diesel::insert(&new)
                            .into(schema::last_weather_search::table)
                            .execute(&conn)
                        {
                            crit!(log, "Failed to update weather location: {:?}", e);
                        }
                    }
                    new_loc
                } else {
                    debug!(log, "No location found");
                    return "Invalid `.weather` syntax, try: `.help weather`".into();
                },
            )
        }
    };

    // Search for geocoding for location
    let reqwest_client = Client::new();
    let reqwest_client = if let Err(e) = reqwest_client {
        crit!(log, "failed to created reqwest client: {:?}", e);
        panic!("")
    } else {
        reqwest_client.unwrap()
    };

    let mut res = reqwest_client
        .get(&format!(
            "{}?key={}&location={}",
            GEOCODING_API_BASE,
            cfg.geocoding_key.as_ref().unwrap(),
            location
        ))
        .send();
    let (latitude, longitude) = if let Err(e) = res {
        crit!(log, "Failed to query geocoding API: {}", e);
        panic!("")
    } else if !res.as_ref().unwrap().status().is_success() {
        crit!(
            log,
            "Failed to query geocoding API: {}",
            res.unwrap().status()
        );
        panic!("")
    } else {
        let mut body = String::new();
        res.unwrap().read_to_string(&mut body).unwrap();

        let json: Value = de::from_str(&body).unwrap();
        (
            json.pointer("/results/0/locations/0/latLng/lat")
                .unwrap()
                .as_f64()
                .unwrap(),
            json.pointer("/results/0/locations/0/latLng/lng")
                .unwrap()
                .as_f64()
                .unwrap(),
        )
    };
    trace!(log, "lat: {}; lng: {}", latitude, longitude);

    /*
    let api_client = ApiClient::new(&reqwest_client);

	let mut blocks = vec![ExcludeBlock::Daily, ExcludeBlock::Alerts];

    let forecast_request = ForecastRequestBuilder::new(api_key, LAT, LONG)
        .exclude_block(ExcludeBlock::Hourly)
        .exclude_blocks(&mut blocks)
        .extend(ExtendBy::Hourly)
        .lang(Lang::Arabic)
        .units(Units::Imperial)
        .build();
	// dont forget to get header for remaining api call num

    */
    unimplemented!()
}
