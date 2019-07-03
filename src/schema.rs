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

table! {
    geocode_cache (location) {
        location -> Text,
        latitude -> Float,
        longitude -> Float,
        reverse_location -> Text,
    }
}

table! {
    location_cache (server, nick) {
        server -> Text,
        nick -> Text,
        location -> Text,
    }
}

table! {
    pending_tells (date, message) {
        date -> Text,
        server_addr -> Text,
        channel -> Nullable<Text>,
        source_nick -> Text,
        target_nick -> Text,
        message -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    geocode_cache,
    location_cache,
    pending_tells,
);
