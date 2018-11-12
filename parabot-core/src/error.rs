// Copyright (C) 2018  ParadoxSpiral
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

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// A module name occured more than once in the same channel, these need to be unique because
    /// they are used to dynamically load the correct module
    ModuleDuplicate,
    /// A module name could not be resolved to a module
    ModuleNotFound(String),
    /// A module trigger (aka a required prefix such as [.]weather) occured more than once
    TriggerDuplicate,

    ConfigFormat(::toml::de::Error),
    Sql(::diesel::result::Error),
    SqlConnection(::diesel::result::ConnectionError),
    Io(::std::io::Error),
    Irc(::irc::error::IrcError),
}

impl From<::toml::de::Error> for Error {
    #[inline]
    fn from(e: ::toml::de::Error) -> Error {
        Error::ConfigFormat(e)
    }
}

impl From<::std::io::Error> for Error {
    #[inline]
    fn from(e: ::std::io::Error) -> Error {
        Error::Io(e)
    }
}

impl From<::diesel::result::Error> for Error {
    #[inline]
    fn from(e: ::diesel::result::Error) -> Error {
        Error::Sql(e)
    }
}

impl From<::diesel::result::ConnectionError> for Error {
    #[inline]
    fn from(e: ::diesel::result::ConnectionError) -> Error {
        Error::SqlConnection(e)
    }
}

impl From<::irc::error::IrcError> for Error {
    #[inline]
    fn from(e: ::irc::error::IrcError) -> Error {
        Error::Irc(e)
    }
}
