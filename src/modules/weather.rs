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

use chrono::Duration;
use chrono::prelude::*;
use chrono_tz::Tz;
use diesel;
use diesel::prelude::*;
use forecast::{Alert, ApiClient, ApiResponse, DataPoint, ExcludeBlock, ExtendBy,
               ForecastRequestBuilder, Units};
use irc::client::prelude::*;
use parking_lot::RwLock;
use regex::Regex;
use reqwest::Client;
use reqwest::header::{qitem, AcceptEncoding, Encoding};
use serde_json::de;
use serde_json::Value;
use slog::Logger;

use std::collections::HashMap;
use std::io::Read;

use config::{Config, ServerCfg};
use errors::*;
use models;
use schema;
use schema::location_cache::dsl as lc_dsl;
use schema::geocode_cache::dsl as gc_dsl;

const GEOCODING_API_BASE: &str = "http://www.mapquestapi.com/geocoding/v1/address";
const REVERSE_GEOCODING_API_BASE: &str = "http://www.mapquestapi.com/geocoding/v1/reverse";

lazy_static!{
    static ref LOCATION_CACHE: RwLock<HashMap<(String, String), String>> = {
        RwLock::new(HashMap::new())
    };
    static ref GEOCODING_CACHE: RwLock<HashMap<String, (f32, f32, String)>> = {
        RwLock::new(HashMap::new())
    };
}

// Read DB to get init values
pub fn init(cfg: &Config, log: &Logger) -> Result<()> {
    let mut lc = LOCATION_CACHE.write();
    let mut gc = GEOCODING_CACHE.write();
    for srv in &cfg.servers {
        let (locations, geocodes) = super::with_database(srv, |db| {
            Ok((
                lc_dsl::location_cache
                    .filter(lc_dsl::server.eq(&srv.address))
                    .load::<models::Location>(db)?,
                gc_dsl::geocode_cache.load::<models::Geocode>(db)?,
            ))
        })?;

        info!(log, "Location cache: {:?}", &locations);
        for q in locations {
            lc.insert((srv.address.clone(), q.nick.clone()), q.location);
        }
        lc.shrink_to_fit();
        info!(log, "Geocode cache: {:?}", &geocodes);
        for g in geocodes {
            gc.insert(
                g.location.clone(),
                (g.latitude, g.longitude, g.reverse_location),
            );
        }
        gc.shrink_to_fit();
    }
    Ok(())
}

pub fn handle(
    cfg: &ServerCfg,
    srv: &IrcServer,
    log: &Logger,
    msg: &str,
    nick: &str,
) -> Result<String> {
    let (range, hours, days, location) = {
        // Use last location
        if msg.is_empty() {
            (0..=0, false, false, {
                if let Some(cached) = LOCATION_CACHE
                    .read()
                    .get(&(cfg.address.clone(), nick.to_owned()))
                {
                    cached.clone()
                } else {
                    return Ok("You have never used `.weather` before, try `.help weather`".into());
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
                return Ok("Invalid `.weather` syntax, try: `.help weather`".into());
            };

            let range = if let Some(d) = captures.name("digits") {
                let n = d.as_str().parse::<usize>().unwrap();
                n..=n
            } else if let (Some(x), Some(y)) = (captures.name("range_x"), captures.name("range_y"))
            {
                let x = x.as_str().parse::<usize>().unwrap();
                let y = y.as_str().parse::<usize>().unwrap();
                x..=y
            } else {
                0..=0
            };
            let h = captures.name("h").is_some() || captures.name("hours").is_some();
            let d = captures.name("d").is_some() || captures.name("days").is_some();
            if range.end > 168 && h || range.end > 7 && d {
                return Ok("Weather data is only available for the next 168h or 7d.".to_owned());
            }

            (
                range,
                h,
                d,
                match (
                    captures.name("inner_location"),
                    captures.name("outer_location"),
                ) {
                    (Some(loc), None) | (None, Some(loc)) => {
                        let new_loc = loc.as_str().trim().to_owned();
                        // Potentially update the cache and DB
                        let mut cache = LOCATION_CACHE.write();
                        if let Some(cached_loc) =
                            cache.get(&(cfg.address.clone(), nick.to_owned())).cloned()
                        {
                            // Only update if the location actually changed
                            if cached_loc.to_lowercase() != new_loc.to_lowercase() {
                                trace!(log, "Updating Cache/DB");
                                cache.remove(&(cfg.address.clone(), nick.to_owned()));
                                cache
                                    .insert((cfg.address.clone(), nick.to_owned()), new_loc.clone());
                                drop(cache);

                                super::with_database(cfg, |db| {
                                    diesel::update(
                                        lc_dsl::location_cache
                                            .filter(lc_dsl::server.eq(&cfg.address))
                                            .filter(lc_dsl::nick.eq(nick)),
                                    ).set(lc_dsl::location.eq(new_loc.clone()))
                                        .execute(db)?;
                                    Ok(())
                                })?;
                            } else {
                                trace!(log, "No location update needed")
                            }
                        } else {
                            trace!(log, "Inserting into Cache/DB");
                            cache.insert((cfg.address.clone(), nick.to_owned()), new_loc.clone());
                            drop(cache);

                            let new = models::NewLocation {
                                server: &cfg.address,
                                nick: nick,
                                location: &*new_loc,
                            };
                            super::with_database(cfg, |db| {
                                diesel::insert(&new)
                                    .into(schema::location_cache::table)
                                    .execute(db)?;
                                Ok(())
                            })?;
                        }
                        new_loc
                    }
                    (Some(_), Some(_)) => unreachable!(),
                    (None, None) => {
                        let cache = LOCATION_CACHE.read();
                        if let Some(cached_loc) =
                            cache.get(&(cfg.address.clone(), nick.to_owned())).cloned()
                        {
                            cached_loc
                        } else {
                            debug!(log, "No location found");
                            return Ok("Invalid `.weather` syntax, try: `.help weather`".into());
                        }
                    }
                },
            )
        }
    };

    // Try to get geocode for location from cache, or request from API
    let cache = GEOCODING_CACHE.read();
    let (latitude, longitude, reverse_location, client);
    if let Some((lat, lng, revl)) = cache.get(&location.to_lowercase()).cloned() {
        trace!(
            log,
            "Got geocode from cache: lat: {}; lng: {}, revl: {}",
            lat,
            lng,
            revl
        );
        latitude = lat;
        longitude = lng;
        reverse_location = revl.clone();
        client = None;
    } else {
        let reqwest_client = Client::new();
        let json: Value = reqwest_client
            .get(&format!(
                "{}?key={}&location={}",
                GEOCODING_API_BASE,
                cfg.geocoding_key.as_ref().unwrap(),
                location
            ))
            .header(AcceptEncoding(vec![qitem(Encoding::Gzip)]))
            .send()?
            .json()?;

        drop(cache);

        let status = json.pointer("/info/statuscode").unwrap().as_u64().unwrap();
        let messages = json.pointer("/info/messages").unwrap().as_array().unwrap();
        if status == 403 {
            crit!(
                log,
                "Geocoding API key probably reached max quota: {:?}",
                messages
            );
            return Ok(
                "Sorry, the geocoding API key seems to have reached its max qouta; \
                 It resets each month."
                    .to_owned(),
            );
        } else if status != 0 {
            crit!(log, "Geocoding request failed");
            bail!("Geocoding request failed: {:?}", messages);
        }
        let lat = json.pointer("/results/0/locations/0/latLng/lat")
            .unwrap()
            .as_f64()
            .unwrap() as f32;
        let lng = json.pointer("/results/0/locations/0/latLng/lng")
            .unwrap()
            .as_f64()
            .unwrap() as f32;

        let quality = json.pointer("/results/0/locations/0/geocodeQualityCode")
            .unwrap()
            .as_str()
            .unwrap();
        trace!(log, "Geocode quality: {}", quality);

        // Reverse geocode lookup to get location to reply with
        let json: Value = reqwest_client
            .get(&format!(
                "{}?key={}&location={},{}",
                REVERSE_GEOCODING_API_BASE,
                cfg.geocoding_key.as_ref().unwrap(),
                lat,
                lng
            ))
            .header(AcceptEncoding(vec![qitem(Encoding::Gzip)]))
            .send()?
            .json()?;

        let status = json.pointer("/info/statuscode").unwrap().as_u64().unwrap();
        if status == 403 {
            crit!(
                log,
                "Geocoding API key probably reached max quota: {:?}",
                messages
            );
            return Ok(
                "Sorry, the geocoding API key seems to have reached its max qouta; \
                 It resets each month."
                    .to_owned(),
            );
        } else if status != 0 {
            bail!("Reverse geocoding request failed: {:?}", messages);
        }
        let city = json.pointer("/results/0/locations/0/adminArea5")
            .unwrap()
            .as_str()
            .unwrap();
        let county = json.pointer("/results/0/locations/0/adminArea4")
            .unwrap()
            .as_str()
            .unwrap();
        let state = json.pointer("/results/0/locations/0/adminArea3")
            .unwrap()
            .as_str()
            .unwrap();
        let country = json.pointer("/results/0/locations/0/adminArea1")
            .unwrap()
            .as_str()
            .unwrap();
        let mut revl = String::new();
        if city != "" {
            revl.push_str(&format!("{}, ", city));
        }
        if state != "" {
            revl.push_str(&format!("{}, ", state));
        } else if county != "" {
            revl.push_str(&format!("{}, ", county));
        }
        revl.push_str(country);

        GEOCODING_CACHE
            .write()
            .insert(location.to_lowercase().to_owned(), (lat, lng, revl.clone()));

        let new = models::NewGeocode {
            location: &location.to_lowercase(),
            latitude: lat,
            longitude: lng,
            reverse_location: &revl.clone(),
        };
        super::with_database(cfg, |db| {
            diesel::insert(&new)
                .into(schema::geocode_cache::table)
                .execute(db)?;
            Ok(())
        })?;

        trace!(log, "Got geocode from API: lat: {}; lng: {}", lat, lng);
        latitude = lat;
        longitude = lng;
        reverse_location = revl;
        client = Some(reqwest_client);
    }

    // future, n, hours, days, location
    let client = client.or(Some(Client::new())).unwrap();
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
    let mut res = api_client.get_forecast(builder.build())?;
    if !res.status().is_success() {
        bail!("Failed to query weather API");
    }

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

    let format_data_point = |out: &mut String, dp: &DataPoint| {
        if let Some(ref s) = dp.summary {
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
        if let Some(pp) = dp.precip_probability {
            if pp > 0.049f64 {
                if dp.wind_speed.is_some() {
                    out.push_str(&format!(
                        "{}% chance of {}; ",
                        (pp * 100f64).round(),
                        format!("{:?}", dp.precip_type.as_ref().unwrap()).to_lowercase()
                    ));
                } else {
                    out.push_str(&format!(
                        "{}% chance of {}",
                        (pp * 100f64).round(),
                        format!("{:?}", dp.precip_type.as_ref().unwrap()).to_lowercase()
                    ));
                }
            }
        }
        if let Some(ws) = dp.wind_speed {
            out.push_str(&format!("{}km/h wind speed", ws));
        }
    };
    let format_alerts = |out: &mut String, alerts: &Option<Vec<Alert>>| -> Result<()> {
        if let Some(ref alerts) = *alerts {
            let utc_now = Utc::now();
            let timezone: Tz = res.timezone.parse().unwrap();
            let range_adjustment = if days {
                Duration::days(range.start as _)
            } else {
                Duration::hours(range.start as _)
            };
            let adjusted_request_time = utc_now.with_timezone(&timezone) + range_adjustment;
            let mut num = 0;
            for (n, a) in alerts.iter().enumerate() {
                let mut expired = false;
                if let Some(expires) = a.expires {
                    if adjusted_request_time > timezone.timestamp(expires as _, 0) {
                        expired = true;
                        trace!(log, "Expired alert");
                    }
                }
                if !expired {
                    num += 1;
                    super::send_segmented_message(
                        cfg,
                        srv,
                        log,
                        nick,
                        &format!(
                            "\x02{}: {}\x02 in {} …]; <{}>",
                            n + 1,
                            a.title,
                            &a.regions.iter().take(13).fold("[".to_owned(), |acc, reg| acc + reg + ","),
                            a.description
                        ),
                    )?;
                }
            }
            if num != 0 {
                out.push_str(&format!("; PMed {} alert(s)", num));
            }
        }
        Ok(())
    };

    let mut formatted = String::new();
    if range.start == range.end && range.start == 0 {
        let data;
        if days {
            data = &res.daily.as_ref().unwrap().data[0];
            formatted.push_str(&format!("Today's weather in {} is ", reverse_location));
            format_data_point(&mut formatted, data);
        } else {
            data = res.currently.as_ref().unwrap();
            formatted.push_str(&format!("Current weather in {} is ", reverse_location));
            format_data_point(&mut formatted, data);
        }
    } else if range.start == range.end {
        let data;
        if days {
            data = &res.daily.as_ref().unwrap().data[range.start];
            formatted.push_str(&format!(
                "Weather in {}d in {} is ",
                range.start,
                reverse_location
            ));
            format_data_point(&mut formatted, data);
        } else {
            data = &res.hourly.as_ref().unwrap().data[range.start];
            formatted.push_str(&format!(
                "Weather in {}h in {} is ",
                range.start,
                reverse_location
            ));
            format_data_point(&mut formatted, data);
        }
    } else {
        let data;
        if hours {
            data = res.hourly.as_ref().unwrap();
            formatted.push_str(&format!(
                "Weather in the next {}-{}h in {}: ",
                range.start,
                range.end,
                reverse_location
            ));
        } else {
            data = res.daily.as_ref().unwrap();
            formatted.push_str(&format!(
                "Weather in the next {}-{}d in {}: ",
                range.start,
                range.end,
                reverse_location
            ));
        }
        for (n, data) in data.data[range.start..=range.end]
            .into_iter()
            .cloned()
            .enumerate()
        {
            formatted.push_str(&format!("\x02{}:\x02 ", n + range.start));
            format_data_point(&mut formatted, &data);
            if n + range.start != range.end {
                formatted.push_str("--- ");
            }
        }
    }
    format_alerts(&mut formatted, &res.alerts)?;

    Ok(formatted)
}
