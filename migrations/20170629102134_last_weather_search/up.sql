CREATE TABLE last_weather_search (
  server TEXT NOT NULL,
  nick TEXT NOT NULL,
  location TEXT NOT NULL,
  PRIMARY KEY (server, nick)
);
