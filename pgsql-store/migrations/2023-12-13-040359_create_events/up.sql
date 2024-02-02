-- Your SQL goes here
CREATE TABLE events (
    cid VARCHAR(70) NOT NULL CONSTRAINT events_pk PRIMARY KEY,
    prev VARCHAR(70),
    genesis VARCHAR(70) NOT NULL,
    blocks BYTEA [] NOT NULL CHECK (
        blocks <> '{}'
        and array_position(blocks, null) IS NULL
    )
);

CREATE TABLE streams (
    stream_id VARCHAR(70) NOT NULL CONSTRAINT streams_pk PRIMARY KEY,
    dapp_id uuid NOT NULL,
    tip VARCHAR(70) NOT NULL,
    account VARCHAR(100),
    model_id VARCHAR(70),
    content JSONB NOT NULL
);

CREATE TABLE index_folders (
    stream_id VARCHAR(70) NOT NULL CONSTRAINT index_folders_pk PRIMARY KEY,
    tip VARCHAR(70) NOT NULL,
    signal JSONB
);

CREATE INDEX index_folders_tip_idx ON index_folders (signal);