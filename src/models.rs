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

use super::schema::*;

#[derive(Debug, Queryable)]
pub struct PendingTell {
    pub date: String,
    pub server_addr: String,
    pub channel: Option<String>,
    pub source_nick: String,
    pub target_nick: String,
    pub message: String,
}

#[derive(Debug, Insertable)]
#[table_name = "pending_tells"]
pub struct NewPendingTell<'a> {
    pub date: &'a str,
    pub server_addr: &'a str,
    pub channel: Option<&'a str>,
    pub source_nick: &'a str,
    pub target_nick: &'a str,
    pub message: &'a str,
}

#[derive(Debug, Queryable)]
pub struct Location {
    pub server: String,
    pub nick: String,
    pub location: String,
}

#[derive(Debug, Insertable)]
#[table_name = "location_cache"]
pub struct NewLocation<'a> {
    pub server: &'a str,
    pub nick: &'a str,
    pub location: &'a str,
}

#[derive(Debug, Queryable)]
pub struct Geocode {
    pub location: String,
    pub latitude: f32,
    pub longitude: f32,
    pub reverse_location: String,
}

#[derive(Debug, Insertable)]
#[table_name = "geocode_cache"]
pub struct NewGeocode<'a> {
    pub location: &'a str,
    pub latitude: f32,
    pub longitude: f32,
    pub reverse_location: &'a str,
}
