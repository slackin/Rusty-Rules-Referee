-- Add auth column to clients table to persist UrbanTerror auth identity
ALTER TABLE clients ADD COLUMN auth TEXT NOT NULL DEFAULT '';
