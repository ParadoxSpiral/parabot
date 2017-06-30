CREATE TABLE location_cache (
  server TEXT NOT NULL,
  nick TEXT NOT NULL,
  location TEXT NOT NULL,
  PRIMARY KEY (server, nick)
);
