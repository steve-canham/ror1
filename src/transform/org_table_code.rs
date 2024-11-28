use sqlx::{Pool, Postgres};

pub async fn recreate_org_tables (pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {

    let table_sql  = r#"drop table if exists org.core_data;
    create table org.core_data
    (
          id                varchar     not null primary key
        , ror_full_id       varchar     not null
        , ror_name          varchar     not null	
        , status            varchar     not null default 1
        , established       int         null
        , location          varchar     null
        , country_code      varchar     null
    );"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;


    let table_sql  = r#"drop table if exists org.admin_data;
    create table org.admin_data
    (
          id                varchar     not null primary key
        , ror_name          varchar     not null	              
        , n_locs            int         not null default 0
        , n_labels          int         not null default 0
        , n_aliases         int         not null default 0
        , n_acronyms        int         not null default 0
        , n_names           int         not null default 0
        , n_langcodes       int         not null default 0
        , n_isni            int         not null default 0
        , n_grid            int         not null default 0
        , n_fundref         int         not null default 0
        , n_wikidata        int         not null default 0
        , n_wikipaedia      int         not null default 0
        , n_website         int         not null default 0
        , n_types           int         not null default 0
        , n_relrels         int         not null default 0
        , n_parrels         int         not null default 0
        , n_chrels          int         not null default 0
        , n_sucrels         int         not null default 0
        , n_predrels        int         not null default 0
        , n_doms            int         not null default 0
        , created           date        not null
        , cr_schema         varchar     not null
        , last_modified     date        not null
        , lm_schema        varchar      not null  
    );"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;


    let table_sql  = r#"drop table if exists org.names;
    create table org.names
    (
          id                varchar     not null
        , value             varchar     not null  
        , name_type         int         not null 
        , is_ror_name       bool        not null default false
        , lang_code         varchar     null
        , script_code       varchar     null
    );
    create index names_idx on org.names(id);"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;


    let table_sql  = r#"drop table if exists org.locations;
    create table org.locations
    (
          id                varchar     not null
        , ror_name          varchar     not null
        , geonames_id       int         null
        , geonames_name     varchar     null	
        , lat               real        null
        , lng               real        null
        , country_code      varchar     null
        , country_name      varchar     null	
    );
    create index locations_idx on org.locations(id);"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;


    let table_sql  = r#"drop table if exists org.external_ids;
    create table org.external_ids
    (
          id                varchar     not null
        , ror_name          varchar     not null	
        , id_type           int         not null
        , id_value          varchar     not null
        , is_preferred      bool        not null default false
    );
    create index external_ids_idx on org.external_ids(id);"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;


    let table_sql  = r#"drop table if exists org.links;
    create table org.links
    (
          id                varchar     not null
        , ror_name          varchar     not null  	  
        , link_type         int         not null
        , link              varchar     not null
    );
    create index links_idx on org.links(id);"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;


    let table_sql  = r#"drop table if exists org.type;
    create table org.type
    (
          id                varchar     not null
        , ror_name          varchar     not null
        , org_type          int         not null
    );  
    create index type_idx on org.type(id);"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;


    let table_sql  = r#"drop table if exists org.relationships;
    create table org.relationships
    (
          id                varchar     not null
        , ror_name          varchar     not null
        , rel_type          int         not null
        , related_id        varchar     not null
        , related_name      varchar     not null
    );  
    create index relationships_idx on org.relationships(id);"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;
 

    let table_sql  = r#"drop table if exists org.domains;
    create table org.domains
    (
          id                varchar     not null
        , ror_name          varchar     not null
        , domain            varchar     not null
    );
    create index domains_idx on org.domains(id);"#;
    sqlx::raw_sql(table_sql).execute(pool).await?;

    Ok(())

}