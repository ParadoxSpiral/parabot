CREATE TABLE geocode_cache (
  location TEXT NOT NULL PRIMARY KEY,
  latitude REAL NOT NULL,
  longitude REAL NOT NULL,
  reverse_location TEXT NOT NULL
);
