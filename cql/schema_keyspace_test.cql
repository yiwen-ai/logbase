CREATE KEYSPACE IF NOT EXISTS logbase_test
WITH replication = { 'class': 'SimpleStrategy', 'replication_factor': '1' };
USE logbase_test;
