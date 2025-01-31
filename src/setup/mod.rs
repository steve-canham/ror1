/**********************************************************************************
* The setup module. Referenced in main by 'mod setup'.
* The two public modules allow integration tests to call into them, to give those
* tests the same DB conection pool and logging capability as the main library.
* The log established by log_helper seems to be available throughout the program
* via a suitable 'use' statement.
***********************************************************************************/

pub mod env_reader;
pub mod log_helper;
mod cli_reader;
mod lup_create_tables;
mod lup_fill_tables;

/**********************************************************************************
* This over-arching 'mod' setup module 
* a) establishes the final collection of parameters, taking into account both 
* environmental and CLI values. 
* b) Unpacks the file name to obtain data version and date, if possible, 
* c) Obtains a database connection pool 
* d) Orchestrates the creation of the lookup and summary schemas.
* It has a collection of unit tests ensuring that the parameter generatiuon process 
* is correct as well as some tests on the regex expression used on the source file.
***********************************************************************************/

use crate::error_defs::{AppError, CustomError};
use chrono::NaiveDate;
use sqlx::postgres::{PgPoolOptions, PgConnectOptions, PgPool};
use sqlx::{Postgres, Pool};
use log::{info, error};
use chrono::Local;
use std::path::PathBuf;
use std::ffi::OsString;
use std::fs;
use std::time::Duration;
use regex::Regex;
use sqlx::ConnectOptions;

#[derive(Debug)]
pub struct CliPars {
    pub data_folder: PathBuf,
    pub source_file: String,
    pub data_version: String,
    pub data_date: String,
    pub flags: Flags, 
}

#[derive(Debug, Clone, Copy)]
pub struct Flags {
    pub import_ror: bool,
    pub process_data: bool,
    pub export_text: bool,
    pub export_csv: bool,
    pub export_full_csv: bool,
    pub create_lookups: bool,
    pub create_summary: bool,
    pub test_run: bool,
}

pub struct InitParams {
    pub data_folder: PathBuf,
    pub log_folder: PathBuf,
    pub output_folder: PathBuf,
    pub source_file_name: String,
    pub output_file_name: String,
    pub data_version: String,
    pub data_date: String,
    pub flags: Flags,
}

pub async fn get_params(args: Vec<OsString>) -> Result<InitParams, AppError> {

    // Called from main as the initial task of the program.
    // Returns a struct that contains the program's parameters.
    // Start by obtaining CLI arguments and reading parameters from .env file.
    
    env_reader::populate_env_vars()?; 
    let cli_pars = cli_reader::fetch_valid_arguments(args)?;

    if cli_pars.flags.create_lookups || cli_pars.flags.create_summary {

       // Any ror data and any other flags or arguments are ignored.

        Ok(InitParams {
            data_folder: PathBuf::new(),
            log_folder: PathBuf::new(),
            output_folder: PathBuf::new(),
            source_file_name: "".to_string(),
            output_file_name: "".to_string(),
            data_version: "".to_string(),
            data_date: "".to_string(),
            flags: cli_pars.flags,
        })
    }
    else {

        // Normal import and / or processing and / or outputting
        // If folder name also given in CL args the CL version takes precedence

        let empty_pb = PathBuf::from("");
        let mut data_folder_good = true;

        let mut data_folder = cli_pars.data_folder;
        if data_folder == empty_pb {
            data_folder =  env_reader::fetch_data_folder();
        }
             
        // Does this folder exist and is it accessible? - If not and the 
        // 'R' (import ror) option is active, raise error and exit program.
                
        if !folder_exists (&data_folder) 
        {   
            data_folder_good = false;
        }
        if !data_folder_good && cli_pars.flags.import_ror { 
            let msg = "Required data folder does not exists or is not accessible";
            let cf_err = CustomError::new(msg);
            return Result::Err(AppError::CsErr(cf_err));
        }

        let mut log_folder = env_reader::fetch_log_folder();
        if log_folder == empty_pb && data_folder_good {
            log_folder = data_folder.clone();
        }
        else {
            if !folder_exists (&log_folder) { 
                fs::create_dir_all(&log_folder)?;
            }
        }

        let mut output_folder = env_reader::fetch_output_folder();
        if output_folder == empty_pb && data_folder_good {
            output_folder = data_folder.clone();
        }
        else {
            if !folder_exists (&output_folder) { 
                fs::create_dir_all(&output_folder)?;
            }
        }
               

        // If source file name given in CL args the CL version takes precedence.
    
        let mut source_file_name= cli_pars.source_file;
        if source_file_name == "" {
            source_file_name =  env_reader::fetch_source_file_name();
            if source_file_name == "" && cli_pars.flags.import_ror {   // Required data is missing - Raise error and exit program.
                let msg = "Source file name not provided in either command line or environment file";
                let cf_err = CustomError::new(msg);
                return Result::Err(AppError::CsErr(cf_err));
             }
        }
        
        let mut data_version = "".to_string();
        let mut data_date = "".to_string();
       
        // If file name conforms to the correct pattern data version and data date can be derived.
        
        if cli_pars.flags.test_run {
            data_version = "v99".to_string();
            data_date = "2030-01-01".to_string()
        }
        else {
            if is_compliant_file_name(&source_file_name) {
                data_version = get_data_version(&source_file_name);
                data_date = get_data_date(&source_file_name);
            }
        }

        if data_version == "".to_string() ||  data_date == "".to_string()     
        {
            // Parsing of file name has not been completely successful, so get the version and date 
            // of the data from the CLI, or failing that the config file.

            data_version= cli_pars.data_version;
            if data_version == "" {
                data_version =  env_reader::fetch_data_version();
                if data_version == "" && cli_pars.flags.import_ror {   // Required data is missing - Raise error and exit program.
                    let msg = "Data version not provided in either command line or environment file";
                    let cf_err = CustomError::new(msg);
                    return Result::Err(AppError::CsErr(cf_err));
                }
            }
        
            data_date = match NaiveDate::parse_from_str(&cli_pars.data_date, "%Y-%m-%d") {
                Ok(_) => cli_pars.data_date,
                Err(_) => "".to_string(),
            };

            if data_date == "" {  
                    let env_date = &env_reader::fetch_data_date();
                    data_date = match NaiveDate::parse_from_str(env_date, "%Y-%m-%d") {
                    Ok(_) => env_date.to_string(),
                    Err(_) => "".to_string(),
                };

                if data_date == "" && cli_pars.flags.import_ror {   // Raise an AppError...required data is missing.
                    let msg = "Data date not provided";
                    let cf_err = CustomError::new(msg);
                    return Result::Err(AppError::CsErr(cf_err));
                }
            }
        }

        // get the output file name - if anywhere it is in the .env variables
        
        let mut output_file_name =  env_reader::fetch_output_file_name();
        if output_file_name == "" {
            output_file_name = format!("{} summary", data_version).to_string()
        }
        let datetime_string = Local::now().format("%m-%d %H%M%S").to_string();
        output_file_name = format!("{} at {}.txt", output_file_name, datetime_string);
  
        // For execution flags read from the environment variables
       
        Ok(InitParams {
            data_folder,
            log_folder,
            output_folder,
            source_file_name,
            output_file_name,
            data_version,
            data_date,
            flags: cli_pars.flags,
        })
    }
}


pub async fn get_db_pool() -> Result<PgPool, AppError> {  

    // Establish DB name and thence the connection string
    // (done as two separate steps to allow for future development).
    // Use the string to set up a connection options object and change 
    // the time threshold for warnings. Set up a DB pool option and 
    // connect using the connection options object.

    let db_name = env_reader::fetch_db_name().unwrap();  // default value of 'ror'
    let db_conn_string = env_reader::fetch_db_conn_string(db_name)?;  
    
    let mut opts: PgConnectOptions = db_conn_string.parse()?;
    opts = opts.log_slow_statements(log::LevelFilter::Warn, Duration::from_secs(3));

    match PgPoolOptions::new()
    .max_connections(5) 
    .connect_with(opts).await {
        Ok(p) => Ok(p),
        Err(e) => {
            error!("An error occured while creating the DB pool: {}", e);
            error!("Check the DB credentials and confirm the database is available");
            return Err(AppError::SqErr(e))
        }
    }
}


fn folder_exists(folder_name: &PathBuf) -> bool {
    let xres = folder_name.try_exists();
    let res = match xres {
        Ok(true) => true,
        Ok(false) => false, 
        Err(_e) => false,           
    };
    res
}


pub async fn create_lup_tables(pool : &Pool<Postgres>) -> Result<(), AppError>
{
    match lup_create_tables::create_tables(pool).await {
        Ok(()) => info!("Tables created for lup schema"),
        Err(e) => {
            error!("An error occured while creating the lup schema tables: {}", e);
            return Err(e)
            },
    };
    match lup_fill_tables::fill_tables(pool).await {
        Ok(()) => info!("Data added to lup tables"),
        Err(e) => {
            error!("An error occured while inserting data into the lup schema tables: {}", e);
            return Err(e)
            },
    };
    Ok(())
}


fn is_compliant_file_name(input: &str) -> bool {
    let file_name_pattern = r#"^v[0-9]+(\.[0-9]+){0,2}(-| )20[0-9]{2}-?[01][0-9]-?[0-3][0-9]"#;
    let re = Regex::new(file_name_pattern).unwrap();
    re.is_match(input)
}

fn get_data_version(input: &str) -> String {

    let version_pattern = r#"^v[0-9]+(\.[0-9]+){0,2}"#;
    let re = Regex::new(version_pattern).unwrap();
    if re.is_match(&input) {
        let caps = re.captures(&input).unwrap();
        caps[0].trim().to_string()
    }
    else {
        "".to_string()
    }
}

fn get_data_date(input: &str) -> String {            
    
    let date_pattern = r#"20[0-9]{2}-?[01][0-9]-?[0-3][0-9]"#;
    let re = Regex::new(date_pattern).unwrap();
    if re.is_match(&input) {
        let caps = re.captures(&input).unwrap();
        let putative_date = caps[0].replace("-", ""); // remove any hyphens
        match NaiveDate::parse_from_str(&putative_date, "%Y%m%d")
        {
            Ok(nd) => nd.to_string(),  // returns as YYY-mm-DD
            Err(_) => "".to_string(),
        }
    } 
    else {
        "".to_string()
    }
}


// Tests
#[cfg(test)]

mod tests {
    use super::*;
   
   // regex tests
   #[test]
   fn check_file_name_regex_works_1 () {
      let test_file_name = "v1.50 2024-12-11.json".to_string();
      assert_eq!(is_compliant_file_name(&test_file_name), true);
      assert_eq!(get_data_version(&test_file_name), "v1.50");
      assert_eq!(get_data_date(&test_file_name), "2024-12-11");
   }

   #[test]
   fn check_file_name_regex_works_2 () {
      let test_file_name = "v1.50-2024-12-11.json".to_string();
      assert_eq!(is_compliant_file_name(&test_file_name), true);
      assert_eq!(get_data_version(&test_file_name), "v1.50");
      assert_eq!(get_data_date(&test_file_name), "2024-12-11");
   }  

   #[test]
   fn check_file_name_regex_works_3 () {
      let test_file_name = "v1.50 20241211.json".to_string();
      assert_eq!(is_compliant_file_name(&test_file_name), true);
      assert_eq!(get_data_version(&test_file_name), "v1.50");
      assert_eq!(get_data_date(&test_file_name), "2024-12-11");
   }

   #[test]
   fn check_file_name_regex_works_4 () {
      let test_file_name = "v1.50-20241211.json".to_string();
      assert_eq!(is_compliant_file_name(&test_file_name), true);
      assert_eq!(get_data_version(&test_file_name), "v1.50");
      assert_eq!(get_data_date(&test_file_name), "2024-12-11");
   }

   #[test]
   fn check_file_name_regex_works_5 () {
      let test_file_name = "v1.50-2024-1211.json".to_string();
      assert_eq!(is_compliant_file_name(&test_file_name), true);
      assert_eq!(get_data_version(&test_file_name), "v1.50");
      assert_eq!(get_data_date(&test_file_name), "2024-12-11");
   }

   #[test]
   fn check_file_name_regex_works_6 () {
      let test_file_name = "v1.59-2025-01-23-ror-data_schema_v2.json".to_string();
      assert_eq!(is_compliant_file_name(&test_file_name), true);
      assert_eq!(get_data_version(&test_file_name), "v1.59");
      assert_eq!(get_data_date(&test_file_name), "2025-01-23");
   }
   
   #[test]
    fn check_file_name_regex_works_7 () {
        let test_file_name = "1.50 2024-12-11.json".to_string();
        assert_eq!(is_compliant_file_name(&test_file_name), false);

        let test_file_name = "v1.50--2024-12-11.json".to_string();
        assert_eq!(is_compliant_file_name(&test_file_name), false);

        let test_file_name = "v1.50  20241211.json".to_string();
        assert_eq!(is_compliant_file_name(&test_file_name), false);

        let test_file_name = "v1.50 20242211.json".to_string();
        assert_eq!(is_compliant_file_name(&test_file_name), false);

        let test_file_name = "v1.50.20241211.json".to_string();
        assert_eq!(is_compliant_file_name(&test_file_name), false);
    }

    // Ensure the parameters are being correctly extracted from the CLI arguments
    // The testing functions need to be async because of the call to get_params.
    // the test therefore uses the async version of the temp_env::with_vars function.
    // This function needs to be awaited to execute.
    // The closure is replaced by an explicitly async block rather than
    // a normal closure. Inserting '||' before or after the 'async' results
    // in multiple complaints from the compiler. The async block can also
    // be replaced by a separate async function and called explicitly.
 
    #[tokio::test]
    async fn check_env_vars_overwrite_blank_cli_values() {

        // Note that in most cases the folder path given must exist, and be 
        // accessible, or get_params will panic and an error will be thrown. 

        temp_env::async_with_vars(
        [
            ("data_folder_path", Some("E:/ROR/data")),
            ("src_file_name", Some("v1.58 20241211.json")),
            ("output_file_name", Some("results 25.json")),
            ("data_version", Some("v1.60")),
            ("data_date", Some("2025-12-11")),

        ],
        async { 
            let args : Vec<&str> = vec!["target/debug/ror1.exe"];
            let test_args = args.iter().map(|x| x.to_string().into()).collect::<Vec<OsString>>();
            let res = get_params(test_args).await.unwrap();
    
            assert_eq!(res.flags.import_ror, true);
            assert_eq!(res.flags.process_data, false);
            assert_eq!(res.flags.export_text, false);
            assert_eq!(res.flags.create_lookups, false);
            assert_eq!(res.flags.create_summary, false);
            assert_eq!(res.data_folder, PathBuf::from("E:/ROR/data"));
            assert_eq!(res.log_folder, PathBuf::from("E:/ROR/logs"));
            assert_eq!(res.output_folder, PathBuf::from("E:/ROR/outputs"));
            assert_eq!(res.source_file_name, "v1.58 20241211.json");
            let lt = Local::now().format("%m-%d %H%M%S").to_string();
            assert_eq!(res.output_file_name, format!("results 25.json at {}.txt", lt));
            assert_eq!(res.data_version, "v1.58");
            assert_eq!(res.data_date, "2024-12-11");
        }
       ).await;

    }


    #[tokio::test]
    async fn check_cli_vars_overwrite_env_values() {

        // Note that the folder path given must exist, 
        // and be accessible, or get_params will panic
        // and an error will be thrown. 

        temp_env::async_with_vars(
        [
            ("data_folder_path", Some("E:/ROR/20241211 1.58 data")),
            ("src_file_name", Some("v1.58 20241211.json")),
            ("data_version", Some("v1.59")),
            ("data_date", Some("2025-12-11")),
            ("output_file_name", Some("results 27.json")),
        ],
        async { 
            let args : Vec<&str> = vec!["target/debug/ror1.exe", "-r", "-p", "-t", "-x",
                                     "-f", "E:/ROR/data", "-d", "2026-12-25", "-s", "schema2 data.json", "-v", "v1.60"];
            let test_args = args.iter().map(|x| x.to_string().into()).collect::<Vec<OsString>>();
            let res = get_params(test_args).await.unwrap();
    
            assert_eq!(res.flags.import_ror, true);
            assert_eq!(res.flags.process_data, true);
            assert_eq!(res.flags.export_text, true);
            assert_eq!(res.flags.export_csv, true);
            assert_eq!(res.flags.create_lookups, false);
            assert_eq!(res.flags.create_summary, false);
            assert_eq!(res.data_folder, PathBuf::from("E:/ROR/data"));
            assert_eq!(res.log_folder, PathBuf::from("E:/ROR/logs"));
            assert_eq!(res.output_folder, PathBuf::from("E:/ROR/outputs"));
            assert_eq!(res.source_file_name, "schema2 data.json");
            let lt = Local::now().format("%m-%d %H%M%S").to_string();
            assert_eq!(res.output_file_name, format!("results 27.json at {}.txt", lt));
            assert_eq!(res.data_version, "v1.60");
            assert_eq!(res.data_date, "2026-12-25");
        }
       ).await;

    }


    #[tokio::test]
    async fn check_cli_vars_with_i_flag() {

        // Note that the folder path given must exist, 
        // and be accessible, or get_params will panic
        // and an error will be thrown. 

        temp_env::async_with_vars(
        [
            ("data_folder_path", Some("E:/ROR/20241211 1.58 data")),
            ("src_file_name", Some("v1.58 20241211.json")),
            ("data_date", Some("2025-12-11")),
            ("output_file_name", Some("results 27.json")),
        ],
        async { 
            let args : Vec<&str> = vec!["target/debug/ror1.exe", "-r", "-p", "-i", 
                                        "-f", "E:/ROR/data", "-d", "2026-12-25", "-s", "schema2 data.json"];
            let test_args = args.iter().map(|x| x.to_string().into()).collect::<Vec<OsString>>();
            let res = get_params(test_args).await.unwrap();
    
            assert_eq!(res.flags.import_ror, false);
            assert_eq!(res.flags.process_data, false);
            assert_eq!(res.flags.export_text, false);
            assert_eq!(res.flags.create_lookups,true);
            assert_eq!(res.flags.create_summary, true);
            assert_eq!(res.data_folder, PathBuf::new());
            assert_eq!(res.log_folder, PathBuf::new());
            assert_eq!(res.output_folder, PathBuf::new());
            assert_eq!(res.source_file_name, "".to_string());
            assert_eq!(res.output_file_name, "".to_string());
            assert_eq!(res.data_version, "".to_string());
            assert_eq!(res.data_date, "".to_string());
        }
       ).await;

    }


    #[tokio::test]
    async fn check_cli_vars_with_a_flag_and_new_win_folders() {

        // Note that the folder path given must exist, 
        // and be accessible, or get_params will panic
        // and an error will be thrown. 

        temp_env::async_with_vars(
        [
            ("data_folder_path", Some("E:\\ROR\\20241211 1.58 data")),
            ("log_folder_path", Some("E:\\ROR\\some logs")),
            ("output_folder_path", Some("E:\\ROR\\dummy\\some outputs")),
            ("src_file_name", Some("v1.58 20241211.json")),
            ("data_date", Some("2025-12-11")),
            ("output_file_name", Some("results 28.json")),
        ],
        async { 
            let args : Vec<&str> = vec!["target/debug/ror1.exe", "-a", "-f", "E:\\ROR\\data", 
                                       "-d", "2026-12-25", "-s", "schema2 data.json", "-v", "v1.60"];
            let test_args = args.iter().map(|x| x.to_string().into()).collect::<Vec<OsString>>();
            let res = get_params(test_args).await.unwrap();
    
            assert_eq!(res.flags.import_ror, true);
            assert_eq!(res.flags.process_data, true);
            assert_eq!(res.flags.export_text, true);
            assert_eq!(res.flags.create_lookups, false);
            assert_eq!(res.flags.create_summary, false);
            assert_eq!(res.data_folder, PathBuf::from("E:/ROR/data"));
            assert_eq!(res.log_folder, PathBuf::from("E:/ROR/some logs"));
            assert_eq!(res.output_folder, PathBuf::from("E:/ROR/dummy/some outputs"));
            assert_eq!(res.source_file_name, "schema2 data.json");
            let lt = Local::now().format("%m-%d %H%M%S").to_string();
            assert_eq!(res.output_file_name, format!("results 28.json at {}.txt", lt));
            assert_eq!(res.data_version, "v1.60");
            assert_eq!(res.data_date, "2026-12-25");
        }
      ).await;

    }
    
    #[tokio::test]
    async fn check_cli_vars_with_a_flag_and_new_posix_folders() {

        // Note that the folder path given must exist, 
        // and be accessible, or get_params will panic
        // and an error will be thrown. 

        temp_env::async_with_vars(
        [
            ("data_folder_path", Some("E:/ROR/data")),
            ("log_folder_path", Some("E:/ROR/some logs 2")),
            ("output_folder_path", Some("E:/ROR/dummy 2/some outputs")),
            ("src_file_name", Some("v1.58 20241211.json")),
            ("data_date", Some("2025-12-11")),
            ("output_file_name", Some("results 28.json")),
        ],
        async { 
            let args : Vec<&str> = vec!["target/debug/ror1.exe", "-a", "-f", "E:/ROR/data", 
                                       "-d", "2026-12-25", "-s", "schema2 data.json", "-v", "v1.60"];
            let test_args = args.iter().map(|x| x.to_string().into()).collect::<Vec<OsString>>();
            let res = get_params(test_args).await.unwrap();
    
            assert_eq!(res.flags.import_ror, true);
            assert_eq!(res.flags.process_data, true);
            assert_eq!(res.flags.export_text, true);
            assert_eq!(res.flags.create_lookups, false);
            assert_eq!(res.flags.create_summary, false);
            assert_eq!(res.data_folder, PathBuf::from("E:/ROR/data"));
            assert_eq!(res.log_folder, PathBuf::from("E:/ROR/some logs 2"));
            assert_eq!(res.output_folder, PathBuf::from("E:/ROR/dummy 2/some outputs"));
            assert_eq!(res.source_file_name, "schema2 data.json");
            let lt = Local::now().format("%m-%d %H%M%S").to_string();
            assert_eq!(res.output_file_name, format!("results 28.json at {}.txt", lt));
            assert_eq!(res.data_version, "v1.60");
            assert_eq!(res.data_date, "2026-12-25");
        }
      ).await;

    }


    #[tokio::test]
    #[should_panic]
    async fn check_wrong_data_folder_panics_if_r() {
    
    temp_env::async_with_vars(
    [
        ("data_folder_path", Some("E:/ROR/20240607 1.47 data")),
        ("log_folder_path", Some("E:/ROR/some logs")),
        ("output_folder_path", Some("E:/ROR/dummy/some outputs")),
        ("src_file_name", Some("v1.58 20241211.json")),
        ("data_date", Some("2025-12-11")),
        ("output_file_name", Some("results 28.json")),
    ],
    async { 
        let args : Vec<&str> = vec!["target/debug/ror1.exe", "-a", "-f", "E:/silly folder name", 
                                    "-d", "2026-12-25", "-s", "schema2 data.json", "-v", "v1.60"];
        let test_args = args.iter().map(|x| x.to_string().into()).collect::<Vec<OsString>>();
        let _res = get_params(test_args).await.unwrap();
        }
      ).await;
    }

    #[tokio::test]
    async fn check_wrong_data_folder_does_not_panic_if_not_r() {
    
        temp_env::async_with_vars(
        [
            ("data_folder_path", Some("E:/ROR/daft data")),
            ("log_folder_path", Some("E:/ROR/some logs")),
            ("output_folder_path", Some("E:/ROR/dummy/some outputs")),
            ("src_file_name", Some("v1.58 20241211.json")),
            ("data_date", Some("2025-12-11")),
            ("output_file_name", Some("results 28.json")),
        ],
        async { 
            let args : Vec<&str> = vec!["target/debug/ror1.exe", "-p", "-f", "E:/ROR/silly folder name", 
                                        "-d", "2026-12-25", "-s", "schema2 data.json", "-v", "v1.60"];
            let test_args = args.iter().map(|x| x.to_string().into()).collect::<Vec<OsString>>();
            let res = get_params(test_args).await.unwrap();
            assert_eq!(res.flags.import_ror, false);
            assert_eq!(res.flags.process_data, true);
            assert_eq!(res.flags.export_text, false);
            assert_eq!(res.flags.create_lookups, false);
            assert_eq!(res.flags.create_summary, false);
            assert_eq!(res.data_folder, PathBuf::from("E:/ROR/silly folder name"));
            assert_eq!(res.log_folder, PathBuf::from("E:/ROR/some logs"));
            assert_eq!(res.output_folder, PathBuf::from("E:/ROR/dummy/some outputs"));
            assert_eq!(res.source_file_name, "schema2 data.json");
            let lt = Local::now().format("%m-%d %H%M%S").to_string();
            assert_eq!(res.output_file_name, format!("results 28.json at {}.txt", lt));
            assert_eq!(res.data_version, "v1.60");
            assert_eq!(res.data_date, "2026-12-25");

            }
        ).await;

        
    }

}


