DROP INDEX asset_storage.assets_search_document_idx;
ALTER TABLE asset_storage.assets DROP COLUMN search_document;
ALTER TABLE asset_storage.assets
    ADD COLUMN search_document TSVECTOR
    GENERATED ALWAYS AS (
        to_tsvector('simple', regexp_replace(display_name, '[^[:alnum:]]+', ' ', 'g'))
    ) STORED;
CREATE INDEX assets_search_document_idx ON asset_storage.assets USING GIN (search_document);
