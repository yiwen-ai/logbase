CREATE KEYSPACE IF NOT EXISTS logbase
WITH replication = { 'class': 'SimpleStrategy', 'replication_factor': '1' };
USE logbase;