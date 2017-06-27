CREATE TABLE pending_tells (
  date TEXT NOT NULL PRIMARY KEY,
  server_addr TEXT NOT NULL,
  channel TEXT,
  source_nick TEXT NOT NULL,
  target_nick TEXT NOT NULL,
  message TEXT NOT NULL
);
