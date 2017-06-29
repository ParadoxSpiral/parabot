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

use hyper::client;
use irc::client::prelude::*;
use regex::Regex;
use slog::Logger;

use config::ServerCfg;

// https://darksky.net/dev/docs/response
#[derive(Deserialize)]
struct ForecastResponse<'a> {
	latitude: f32,
	longtitude: f32,
	timezone: &'a str,
	currently: Option<DataPoint<'a>>,
	minutely: Option<DataBlock<'a>>,
	hourly: Option<DataBlock<'a>>,
	daily: Option<DataBlock<'a>>,
	alerts: Option<&'a [Alert]>,
	flags: Option<Flags<'a>>,
}

// https://darksky.net/dev/docs/response#data-block
#[derive(Deserialize)]
struct DataBlock<'a> {
	data: &'a [DataPoint<'a>],
	summary: Option<&'a str>,
	icon: Option<&'a str>
}

// https://darksky.net/dev/docs/response#data-point
#[derive(Deserialize)]
struct DataPoint<'a> {
	#[serde( rename = "apparentTemperature")]
	apparent_temperature: Option<f32>,
	#[serde( rename = "apparentTemperatureMax")]
	apparent_temperature_max: Option<f32>,
	#[serde( rename = "apparentTemperatureMaxTime")]
	apparent_temperature_max_mime: Option<i64>,
	#[serde( rename = "apparentTemperatureMin")]
	apparent_temperature_min: Option<f32>,
	#[serde( rename = "apparentTemperatureMinTime")]
	apparent_temperature_min_time: Option<i64>,
	#[serde( rename = "cloudCover")]
	cloud_cover: Option<f32>,
	#[serde( rename = "dewPoint")]
	dew_point: Option<f32>,
	humidity: Option<f32>,
	icon: Option<&'a str>,
	#[serde( rename = "moonPhase")]
	moon_phase: Option<f32>,
	#[serde( rename = "nearestStormBearing")]
	nearest_storm_bearing: Option<f32>,
	#[serde( rename = "nearestStormDistance")]
	nearest_storm_distance: Option<usize>,
	ozone: Option<f32>,
	#[serde( rename = "precipAccumulation")]
	precip_accumulation: Option<usize>,
	#[serde( rename = "precipIntensity")]
	precip_intensity: Option<usize>,
	#[serde( rename = "precipIntensityMax")]
	precip_intensity_max: Option<usize>,
	#[serde( rename = "precipIntensityMaxTime")]
	precip_intensity_max_time: Option<i64>,	
	#[serde( rename = "precipProbability")]
	precip_probability: Option<f32>,
	#[serde( rename = "precipType")]
	precip_type: Option<PrecipType>,
	pressure: Option<f32>,
	summary: Option<&'a str>,
	#[serde( rename = "sunriseTime")]
	sunrise_time: Option<i64>,
	#[serde( rename = "sunsetTime")]
	sunset_time: Option<i64>,
	temperature: Option<f32>,
	#[serde( rename = "temperatureMax")]
	temperature_max: Option<f32>,
	#[serde( rename = "temperatureMaxTime")]
	temperature_max_time: Option<i64>,
	#[serde( rename = "temperatureMin")]
	temperature_min: Option<f32>,
	#[serde( rename = "temperatureMinTime")]
	temperature_min_time: Option<i64>,
	time: Option<i64>,
	// TODO: Is this one correct?
	#[serde( rename = "uvIndex")]
	uv_index: Option<usize>,
	#[serde( rename = "uvIndexTime")]
	uv_index_time: Option<i64>,
	visibility: Option<f32>,
	#[serde( rename = "windBearing")]
	wind_bearing: Option<f32>,
	#[serde( rename = "windGust")]
	wind_gust: Option<f32>,
	#[serde( rename = "windGustTime")]
	wind_gust_time: Option<i64>,
	#[serde( rename = "windSpeed")]
	wind_speed: Option<f32>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PrecipType {Rain, Snow, Sleet}

// https://darksky.net/dev/docs/response#alerts
#[derive(Deserialize)]
struct Alert<'a> {
	description: &'a str,
	expires: Option<i64>,
	regions: &'a [&'a str],
	severity: AlertSeverity,
	time: i64,
	title: &'a str,
	uri: &'a str,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AlertSeverity {Advisory, Watch, Warning}

// https://darksky.net/dev/docs/response#flags
#[derive(Deserialize)]
struct Flags<'a> {
	#[serde(rename = "darksky-unavailable")]
	darksky_unavailable: Option<&'a str>,
	sources: &'a [&'a str],
	units: &'a str,
}

pub fn handle(cfg: &ServerCfg, srv: &IrcServer, log: &Logger, msg: &str) -> String {
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

    let (future, n, hours, days, location) = {
        (
            captures.name("plus").is_some(),
            captures.name("digits"),
            captures.name("h").is_some() || captures.name("hours").is_some(),
            captures.name("d").is_some() || captures.name("days").is_some(),
            if let Some(loc) = captures.name("location") {
                loc
            } else {
                debug!(log, "No location found");
                return "Invalid `.weather` syntax, try: `.help weather`".into();
            },
        )
    };

    unimplemented!()
}
