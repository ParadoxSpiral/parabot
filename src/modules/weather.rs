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
use forecast::{Alert, ApiResponse, ApiClient, DataPoint, ForecastRequestBuilder, ExcludeBlock,
               ExtendBy, Units};
use irc::client::prelude::*;
use parking_lot::RwLock;
use regex::Regex;
use reqwest::Client;
use reqwest::header::{Encoding, AcceptEncoding, qitem};
use serde_json::de;
use serde_json::Value;
use slog::Logger;

use std::collections::HashMap;
use std::io::Read;

use config::{Config, ServerCfg};
use models;
use schema;
use schema::location_cache::dsl as lc_dsl;
use schema::geocode_cache::dsl as gc_dsl;

const GEOCODING_API_BASE: &str = "http://www.mapquestapi.com/geocoding/v1/address";

lazy_static!{
    static ref LOCATION_CACHE: RwLock<HashMap<(String, String), String>> = {
        RwLock::new(HashMap::new())
    };
    static ref GEOCODING_CACHE: RwLock<HashMap<String, (f32, f32)>> = {
        RwLock::new(HashMap::new())
    };
}

// Read DB to get init values
pub fn init(cfg: &Config, log: &Logger) {
    let mut lc = LOCATION_CACHE.write();
    let mut gc = GEOCODING_CACHE.write();
    for srv in &cfg.servers {
        let conn = super::establish_database_connection(srv, log);
        let locations = lc_dsl::location_cache
            .filter(lc_dsl::server.eq(&srv.address))
            .load::<models::Location>(&conn);
        let geocodes = gc_dsl::geocode_cache.load::<models::Geocode>(&conn);
        if locations.is_err() {
            crit!(
                log,
                "Failed to load location cache: {:?}",
                locations.as_ref().unwrap_err()
            );
            panic!(
                "Failed to load location cache: {:?}",
                locations.unwrap_err()
            )
        } else {
            debug!(log, "Location cache: {:?}", locations.as_ref().unwrap());
            for q in locations.unwrap() {
                lc.insert((srv.address.clone(), q.nick.clone()), q.location);
            }
            lc.shrink_to_fit();
        };
        if geocodes.is_err() {
            crit!(
                log,
                "Failed to load geocode cache: {:?}",
                geocodes.as_ref().unwrap_err()
            );
            panic!("Failed to load geocode cache: {:?}", geocodes.unwrap_err())
        } else {
            debug!(log, "Geocode cache: {:?}", geocodes.as_ref().unwrap());
            for g in geocodes.unwrap() {
                gc.insert(g.location.clone(), (g.latitude, g.longitude));
            }
            gc.shrink_to_fit();
        };
    }
}

pub fn handle(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, msg: &str, nick: &str) -> String {
    let mut conn = None;
    let (range, hours, days, location) = {
        // Use last location
        if msg.is_empty() {
            (0...0, false, false, {
                if let Some(cached) = LOCATION_CACHE
                    .read()
                    .get(&(cfg.address.clone(), nick.to_owned()))
                {
                    cached.clone()
                } else {
                    return "You have never used `.weather` before, try `.help weather`".into();
                }
            })
        } else {
            // Only compile the regex once
            lazy_static! {
		        static ref REGEX: Regex = Regex::new("\
		        	\\s{0,}\
		        	(?:\
                        (?:(?:(?P<range_x>\\d+)-(?P<range_y>\\d+))\
                             |(?P<digits>\\d+))\
                        \\s{0,}\
			        	(?:(?P<h>h)|(?P<d>d))\
                        \\s{0,}\
                        (?P<inner_location>.+){0,1}\
			        )|\
		        	(?P<outer_location>.+)").unwrap();
		    }

            let captures = if let Some(caps) = REGEX.captures(&msg[1..]) {
                trace!(log, "Weather captures: {:?}", caps);
                caps
            } else {
                debug!(log, "No captures");
                return "Invalid `.weather` syntax, try: `.help weather`".into();
            };

            let range = if let Some(d) = captures.name("digits") {
                let n = d.as_str().parse::<usize>().unwrap();
                n...n
            } else if let (Some(x), Some(y)) =
                (captures.name("range_x"), captures.name("range_y"))
            {
                let x = x.as_str().parse::<usize>().unwrap();
                let y = y.as_str().parse::<usize>().unwrap();
                x...y
            } else {
                0...0
            };
            let h = captures.name("h").is_some() || captures.name("hours").is_some();
            let d = captures.name("d").is_some() || captures.name("days").is_some();
            if range.end > 168 && h || range.end > 7 && d {
                return "Weather data is only available for the next 168h or 7d.".to_owned();
            }

            (
                range,
                h,
                d,
                match (
                    captures.name("inner_location"),
                    captures.name("outer_location"),
                ) {
                    (Some(loc), None) |
                    (None, Some(loc)) => {
                        let new_loc = loc.as_str().trim().to_owned();
                        // Potentially update the cache and DB
                        let mut cache = LOCATION_CACHE.write();
                        if let Some(cached_loc) = cache
                            .get(&(cfg.address.clone(), nick.to_owned()))
                            .cloned()
                        {
                            // Only update if the location actually changed
                            if cached_loc.to_lowercase() != new_loc.to_lowercase() {
                                trace!(log, "Updating Cache/DB");
                                cache.remove(&(cfg.address.clone(), nick.to_owned()));
                                cache.insert((cfg.address.clone(), nick.to_owned()), new_loc.clone());
                                drop(cache);

                                conn = Some(super::establish_database_connection(cfg, log));
                                if let Err(e) = diesel::update(
                                    lc_dsl::location_cache
                                        .filter(lc_dsl::server.eq(&cfg.address))
                                        .filter(lc_dsl::nick.eq(nick)),
                                ).set(lc_dsl::location.eq(new_loc.clone()))
                                    .execute(conn.as_ref().unwrap())
                                {
                                    crit!(log, "Failed to update weather table: {:?}", e);
                                }
                            } else {
                                trace!(log, "No location update needed")
                            }
                        } else {
                            trace!(log, "Inserting into Cache/DB");
                            cache.insert((cfg.address.clone(), nick.to_owned()), new_loc.clone());
                            drop(cache);

                            conn = Some(super::establish_database_connection(cfg, log));
                            let new = models::NewLocation {
                                server: &cfg.address,
                                nick: nick,
                                location: &*new_loc,
                            };
                            if let Err(e) = diesel::insert(&new)
                                .into(schema::location_cache::table)
                                .execute(conn.as_ref().unwrap())
                            {
                                crit!(log, "Failed to update weather table: {:?}", e);
                            }
                        }
                        new_loc
                    }
                    (Some(_), Some(_)) => unreachable!(),
                    (None, None) => {
                        let cache = LOCATION_CACHE.read();
                        if let Some(cached_loc) = cache
                            .get(&(cfg.address.clone(), nick.to_owned()))
                            .cloned()
                        {
                            cached_loc
                        } else {
                            debug!(log, "No location found");
                            return "Invalid `.weather` syntax, try: `.help weather`".into();
                        }
                    }
                },
            )
        }
    };

    // Try to get geocode for location from cache, or request from API
    let cache = GEOCODING_CACHE.read();
    let (latitude, longitude, client) = if let Some(&(lat, lng)) =
        cache.get(&location.to_lowercase())
    {
        drop(cache);
        trace!(log, "Got geocode from cache: lat: {}; lng: {}", lat, lng);
        (lat, lng, None)
    } else {
        let reqwest_client = Client::new();
        let reqwest_client = if let Err(e) = reqwest_client {
            crit!(log, "failed to created reqwest client: {:?}", e);
            panic!("")
        } else {
            reqwest_client.unwrap()
        };

        let res = reqwest_client
            .get(&format!(
                "{}?key={}&location={}",
                GEOCODING_API_BASE,
                cfg.geocoding_key.as_ref().unwrap(),
                location
            ))
            .header(AcceptEncoding(vec![qitem(Encoding::Gzip)]))
            .send();
        if let Err(e) = res {
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

            let status = json.pointer("/info/statuscode").unwrap().as_u64().unwrap();
            let messages = json.pointer("/info/messages").unwrap().as_array().unwrap();
            if status == 0 {
                let lat = json.pointer("/results/0/locations/0/latLng/lat")
                    .unwrap()
                    .as_f64()
                    .unwrap() as f32;
                let lng = json.pointer("/results/0/locations/0/latLng/lng")
                    .unwrap()
                    .as_f64()
                    .unwrap() as f32;

                drop(cache);
                GEOCODING_CACHE
                    .write()
                    .insert(location.to_lowercase().to_owned(), (lat, lng));

                let conn = conn.or_else(|| Some(super::establish_database_connection(cfg, log)))
                    .unwrap();
                let new = models::NewGeocode {
                    location: &location.to_lowercase(),
                    latitude: lat,
                    longitude: lng,
                };
                if let Err(e) = diesel::insert(&new)
                    .into(schema::geocode_cache::table)
                    .execute(&conn)
                {
                    crit!(log, "Failed to update weather table: {:?}", e);
                }

                trace!(log, "Got geocode from API: lat: {}; lng: {}", lat, lng);
                (lat, lng, Some(reqwest_client))
            } else if status == 403 {
                crit!(
                    log,
                    "Geocoding API key probably reached max quota: {:?}",
                    messages
                );
                return "Sorry, the geocoding API key seems to have reached its max qouta; \
                        It resets each month."
                    .to_owned();
            } else {
                crit!(log, "Geocoding reuqest failed: {:?}", messages);
                panic!("")
            }
        }
    };

    // future, n, hours, days, location
    let client = client.or_else(|| Some(Client::new().unwrap())).unwrap();
    let api_client = ApiClient::new(&client);
    let secret = cfg.weather_secret.as_ref().unwrap();
    let mut builder = ForecastRequestBuilder::new(secret, latitude as f64, longitude as f64)
        .units(Units::SI)
        .exclude_block(ExcludeBlock::Minutely);
    if !days && !hours {
        builder = builder
            .exclude_block(ExcludeBlock::Hourly)
            .exclude_block(ExcludeBlock::Daily);
    } else if range.end > 48 && hours {
        builder = builder
            .exclude_block(ExcludeBlock::Currently)
            .extend(ExtendBy::Hourly);
    } else if days || hours {
        builder = builder.exclude_block(ExcludeBlock::Currently);
    }
    let res = api_client.get_forecast(builder.build());
    let mut res = if let Err(e) = res {
        crit!(log, "Failed to query weather API: {}", e);
        panic!("")
    } else if !res.as_ref().unwrap().status().is_success() {
        crit!(
            log,
            "Failed to query weather API: {}",
            res.unwrap().status()
        );
        panic!("")
    } else {
        res.unwrap()
    };

    let api_calls = ::std::str::from_utf8(
        &res.headers().get_raw("X-Forecast-API-Calls").unwrap()[0],
    ).unwrap()
        .parse::<usize>()
        .unwrap();
    info!(
        log,
        "{} remaining weather API calls (assuming free plan) today",
        1000 - api_calls
    );

    let mut body = String::new();
    res.read_to_string(&mut body).unwrap();
    let res: ApiResponse = de::from_str(&body).unwrap();

    let format_data_point = |out: &mut String, dp: DataPoint| {
        if let Some(s) = dp.summary {
            out.push_str(&format!("{}: ", s.to_lowercase()));
        }
        if days {
            if let (Some(tmi), Some(tma)) =
                (dp.apparent_temperature_min, dp.apparent_temperature_max)
            {
                out.push_str(&format!("\x02{}-{}\x02°C; ", tmi, tma));
            } else if let (Some(tmi), Some(tma)) = (dp.temperature_min, dp.temperature_max) {
                out.push_str(&format!("\x02{}-{}\x02°C; ", tmi, tma));
            }
        } else if let Some(t) = dp.apparent_temperature {
            out.push_str(&format!("\x02{}\x02°C; ", t));
        } else if let Some(t) = dp.temperature {
            out.push_str(&format!("\x02{}\x02°C; ", t));
        }
        if let Some(cc) = dp.cloud_cover {
            if let Some(h) = dp.humidity {
                out.push_str(&format!(
                    "{}% cloud cover, {}% humidity; ",
                    (cc * 100f64).round(),
                    (h * 100f64).round()
                ));
            } else {
                out.push_str(&format!("{}% cloud cover; ", (cc * 100f64).round()));
            }
        }
        if let Some(v) = dp.visibility {
            out.push_str(&format!("visibility: {}km; ", v));
        }
        if let Some(pp) = dp.precip_probability {
            if pp > 0.049f64 {
                if dp.wind_gust.is_some() || dp.wind_speed.is_some() {
                    out.push_str(&format!(
                        "{}% chance of {:?}; ",
                        (pp * 100f64).round(),
                        dp.precip_type.unwrap()
                    ));
                } else {
                    out.push_str(&format!(
                        "{}% chance of {:?}",
                        (pp * 100f64).round(),
                        dp.precip_type.unwrap()
                    ));
                }
            }
        }
        if let Some(wg) = dp.wind_gust {
            out.push_str(&format!("Wind gusts: {}km/h", wg));
        }
        if let Some(ws) = dp.wind_speed {
            if dp.wind_gust.is_none() {
                out.push_str(&format!("Wind speed: {}km/h", ws));
            } else {
                out.push_str(&format!(", wind speed: {}km/h", ws));
            }
        }
    };
    let format_alerts =
        |out: &mut String, alerts: Option<Vec<Alert>>| if let Some(alerts) = alerts {
            // FIXME: Check if alert expired in rewuested timeframe
            if alerts.len() > 1 {
                let mut reply = String::new();
                for (n, a) in alerts.iter().enumerate() {
                    reply.push_str(&format!(
                        "{}, severity: {:?} in {:?}; See <{}>; ",
                        n,
                        a.severity,
                        &a.regions,
                        a.uri
                    ));
                }
                super::send_segmented_message(cfg, srv, log, nick, reply.trim_right(), true);
                out.push_str(&format!("; PMed {} alerts", alerts.len()));
            } else {
                out.push_str(&format!(
                    "; Alert, severity: {:?} in {:?}; See <{}>",
                    alerts[0].severity,
                    &alerts[0].regions,
                    alerts[0].uri
                ));
            }
        };

    let mut formatted = String::new();
    if range.start == range.end && range.start == 0 {
        let data;
        if days {
            data = res.daily.unwrap().data[0].clone();
            formatted.push_str(&format!("Today's weather in {} is ", location));
            format_data_point(&mut formatted, data);
        } else {
            data = res.currently.unwrap();
            formatted.push_str(&format!("Current weather in {} is ", location));
            format_data_point(&mut formatted, data);
        }
    } else {
        let data;
        if hours {
            data = res.hourly.unwrap();
            formatted.push_str(&format!(
                "Weather in the next {}-{}h in {}: ",
                range.start,
                range.end,
                location
            ));
        } else {
            data = res.daily.unwrap();
            formatted.push_str(&format!(
                "Weather in the next {}-{}d in {}: ",
                range.start,
                range.end,
                location
            ));
        }
        for (n, data) in data.data[range.start...range.end]
            .into_iter()
            .cloned()
            .enumerate()
        {
            formatted.push_str(&format!("\x02{}:\x02 ", n + range.start));
            format_data_point(&mut formatted, data);
            if n + range.start != range.end {
                formatted.push_str("--- ");
            }
        }
    }
    format_alerts(&mut formatted, res.alerts);

    formatted
}
