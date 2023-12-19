// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "fang_task_state"))]
    pub struct FangTaskState;
}

diesel::table! {
    events (cid) {
        #[max_length = 70]
        cid -> Varchar,
        #[max_length = 70]
        prev -> Nullable<Varchar>,
        #[max_length = 70]
        genesis -> Varchar,
        blocks -> Array<Nullable<Bytea>>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::FangTaskState;

    fang_tasks (id) {
        id -> Uuid,
        metadata -> Jsonb,
        error_message -> Nullable<Text>,
        state -> FangTaskState,
        task_type -> Varchar,
        #[max_length = 64]
        uniq_hash -> Nullable<Bpchar>,
        retries -> Int4,
        scheduled_at -> Timestamptz,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    streams (stream_id) {
        #[max_length = 70]
        stream_id -> Varchar,
        dapp_id -> Uuid,
        #[max_length = 70]
        tip -> Varchar,
        #[max_length = 100]
        account -> Nullable<Varchar>,
        #[max_length = 70]
        model_id -> Nullable<Varchar>,
        content -> Jsonb,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    events,
    fang_tasks,
    streams,
);
