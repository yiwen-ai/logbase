CREATE KEYSPACE IF NOT EXISTS logbase
WITH replication = { 'class': 'NetworkTopologyStrategy', 'replication_factor': '3' };
USE logbase;