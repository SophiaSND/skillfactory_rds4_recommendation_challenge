#![recursion_limit = "4096"]

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Context, Error, Result};
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use json::Json;
use regex::Regex;
// use rusqlite::{Connection, NO_PARAMS};
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use seq_macro::seq;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::prelude::*,
    io::BufReader,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

#[macro_use]
extern crate diesel;
use diesel::RunQueryDsl;
extern crate dotenv;
mod db;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Path to toml config file
    #[structopt(parse(from_os_str), default_value = "../../data")]
    data: PathBuf,
}

pub struct RecordTrain {
    pub id: i32,
    pub overall: i32,
    pub verified: i32,
    pub unix_review_time: i32,
    pub reviewer_name_id: Option<i32>,
    pub summary_id: Option<i32>,
    pub vote: Option<i32>,
    // pub style: Option<String>,
    // pub image: Option<String>,
    pub userid: i32,
    pub itemid: i32,
    pub rating: i32,
}

pub struct RecordImage {
    pub id: i32,
    pub train_id: i32,
    pub image_id: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ItemIdAsin {
    pub itemid: usize,
    pub asin: String,
}

pub struct RecordItemIdAsin {
    pub itemid: i32,
    pub asin: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AlsoViewUsed {
    pub itemid: usize,
    pub also_view_itemid: usize,
    pub is_train: bool,
}

pub struct RecordAlsoView {
    pub itemid: i32,
    pub also_view_itemid: i32,
    pub is_train: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CategoryUsed {
    pub itemid: usize,
    pub category: String,
    pub is_train: bool,
}

pub struct RecordCategory {
    pub itemid: i32,
    pub category_id: i32,
    pub is_train: i32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Train {
    pub overall: f64,
    pub verified: String,
    pub unix_review_time: u64,
    pub reviewer_name: Option<String>,
    pub summary: Option<String>,
    pub vote: Option<String>,
    pub style: Option<String>,
    pub image: Option<String>,
    pub userid: u64,
    pub itemid: u64,
    pub rating: f64,
}

macro_rules! normalize {
    ($value:expr, $map:ident) => {{
        if let Some(id) = $map.get(&$value) {
            *id as i32
        } else {
            let id = $map.len();
            $map.insert($value.clone(), id as i32);
            id as i32
        }
    }};
}

macro_rules! normalize_opt {
    ($value:expr, $map:ident) => {
        $value.map(|s| normalize!(s, $map))
    };
}

macro_rules! insert_into {
    ($table:expr, $conn:expr, $source:expr, $type:ty, $block:block) => {
        {
            let pbar = ProgressBar::new($source.len() as u64);
            pbar.set_style(
                ProgressStyle::default_bar()
                .template("{elapsed_precise}/{eta_precise} [{bar:60.cyan/blue}] {pos}/{len} {wide_msg}")
                .progress_chars("=>-"),
            );
            pbar.enable_steady_tick(125);
            pbar.println(format!("insert into {} . . .", stringify!($table)));
            let chunk_size = 256; // обеспечивает минимальное значения (время компяляции + время выполнения)
            for chunk in &$source.into_iter().chunks(chunk_size) {
                let mut vec_new = Vec::<$type>::new();

                let vec_chunk = chunk.collect::<Vec<_>>();

                seq!(i in 0..256 {
                    if vec_chunk.len() > i {

                        let new_rec = $block;
                        vec_new.push(new_rec);
                    };
                });
                diesel::insert_into($table)
                    .values(&vec_new)
                    .execute(&$conn)
                    .context(format!("insert into {}", stringify!($table)))?;
                pbar.inc(vec_chunk.len() as u64);
            }
            pbar.finish();
        }
    };
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NormalizedUsed {
    pub itemid: usize,
    pub brand: Option<String>,
    pub description: Option<String>,
    pub title: Option<String>,
    pub main_cat: Option<String>,
    pub price: Option<String>,
    pub is_train: bool,
}

pub struct RecordItem {
    pub itemid: i32,
    pub brand_id: Option<i32>,
    pub description_id: Option<i32>,
    pub title_id: Option<i32>,
    pub main_cat_id: Option<i32>,
    pub price_id: Option<i32>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_total = std::time::Instant::now();
    pretty_env_logger::init_timed();
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "warn");
    }
    {
        let opt = Opt::from_args();
        let data_dir = PathBuf::from(&opt.data);

        {
            let filepath = get_filepath(&data_dir, "train.csv.zip");
            let contents = unzip(&filepath).context(format!("{:?}", filepath))?;
            {
                let start = std::time::Instant::now();
                let buffered_reader = BufReader::new(contents.as_bytes());
                let mut dic_reviewer_name = HashMap::<String, i32>::new();
                let mut dic_summary = HashMap::<String, i32>::new();
                let mut dic_image = HashMap::<String, i32>::new();
                let mut vec_main = Vec::<RecordTrain>::new();
                let mut vec_image = Vec::<RecordImage>::new();
                let mut rdr = csv::Reader::from_reader(buffered_reader);
                let mut style_keys = HashSet::new();
                for (i, result) in rdr.deserialize().enumerate() {
                    let record: Train = result?;
                    let itemid = record.itemid as i32;
                    let userid = record.userid as i32;
                    let rating = record.rating as i32;
                    let overall = record.overall as i32;
                    let verified: i32 = if record.verified == "True" { 1 } else { 0 };
                    let unix_review_time = record.unix_review_time as i32;
                    let reviewer_name_id = normalize_opt!(record.reviewer_name, dic_reviewer_name);
                    let summary_id = normalize_opt!(record.summary, dic_summary);
                    let vote = if let Some(vote) = record.vote {
                        lazy_static::lazy_static! {
                            static ref RE_COMMA: Regex = Regex::new(",").unwrap();
                        }
                        let vote = RE_COMMA.replace_all(&vote, "").to_string();
                        Some(vote.parse::<i32>().context(vote)?)
                    } else {
                        None
                    };
                    if let Some(style) = record.style {
                        parse_style(&style, &mut style_keys, i)?;
                    }
                    let id = vec_main.len() as i32;
                    if let Some(image) = record.image {
                        let train_id = id;
                        lazy_static::lazy_static! {
                            static ref RE_QUOTE: Regex = Regex::new("'").unwrap();
                        }
                        let image = RE_QUOTE.replace_all(&image, "\"").to_string();

                        let json = Json::from_str(&image, format!("[{}].image", i))?;
                        for image in json.iter_vec().context("json.iter_vec()")? {
                            if let Ok(image) = image.as_string() {
                                let image_id = normalize!(image, dic_image);
                                let id = vec_image.len() as i32;
                                vec_image.push(RecordImage {
                                    id,
                                    train_id,
                                    image_id,
                                });
                            }
                        }
                    }
                    vec_main.push(RecordTrain {
                        id,
                        overall,
                        verified,
                        unix_review_time,
                        reviewer_name_id,
                        summary_id,
                        vote,
                        userid,
                        itemid,
                        rating,
                    });
                }
                println!(
                    "{}, vec_main: {}, images: {}, style_key: {:?}",
                    arrange_millis::get(
                        std::time::Instant::now().duration_since(start).as_millis()
                    ),
                    vec_main.len(),
                    dic_image.len(),
                    style_keys,
                );

                let conn = db::establish_connection();

                insert_into!(
                    db::dic_reviewer_name::table,
                    conn,
                    dic_reviewer_name,
                    db::NewDicReviewerName,
                    {
                        db::NewDicReviewerName {
                            id: &vec_chunk[i].1,
                            value: vec_chunk[i].0.as_str(),
                        }
                    }
                );

                insert_into!(
                    db::dic_summary::table,
                    conn,
                    dic_summary,
                    db::NewDicSummary,
                    {
                        db::NewDicSummary {
                            id: &vec_chunk[i].1,
                            value: vec_chunk[i].0.as_str(),
                        }
                    }
                );

                insert_into!(db::dic_image::table, conn, dic_image, db::NewDicImage, {
                    db::NewDicImage {
                        id: &vec_chunk[i].1,
                        value: vec_chunk[i].0.as_str(),
                    }
                });

                insert_into!(db::image::table, conn, vec_image, db::NewImage, {
                    db::NewImage {
                        id: &vec_chunk[i].id,
                        train_id: &vec_chunk[i].train_id,
                        image_id: &vec_chunk[i].image_id,
                    }
                });

                insert_into!(db::train::table, conn, vec_main, db::NewTrain, {
                    db::NewTrain {
                        id: &vec_chunk[i].id,
                        userid: &vec_chunk[i].userid,
                        itemid: &vec_chunk[i].itemid,
                        rating: &vec_chunk[i].rating,
                        overall: &vec_chunk[i].overall,
                        verified: &vec_chunk[i].verified,
                        unix_review_time: &vec_chunk[i].unix_review_time,
                        reviewer_name_id: if let Some(v) = &vec_chunk[i].reviewer_name_id {
                            Some(v)
                        } else {
                            None
                        },
                        summary_id: if let Some(v) = &vec_chunk[i].summary_id {
                            Some(v)
                        } else {
                            None
                        },
                        vote: if let Some(v) = &vec_chunk[i].vote {
                            Some(v)
                        } else {
                            None
                        },
                    }
                });
            }
        }

        {
            let filepath = get_filepath(&data_dir, "normalized_used.csv.zip");
            let contents = unzip(&filepath).context(format!("{:?}", filepath))?;
            {
                let start = std::time::Instant::now();
                let mut brands = HashMap::<String, i32>::new();
                let mut descriptions = HashMap::<String, i32>::new();
                let mut prices = HashMap::<String, i32>::new();
                let mut titles = HashMap::<String, i32>::new();
                let mut main_cats = HashMap::<String, i32>::new();
                let mut vec_main = Vec::<RecordItem>::new();
                let buffered_reader = BufReader::new(contents.as_bytes());
                let mut rdr = csv::Reader::from_reader(buffered_reader);
                for result in rdr.deserialize() {
                    let record: NormalizedUsed = result?;
                    let brand_id = normalize_opt!(record.brand, brands);
                    let description_id = normalize_opt!(record.description, descriptions);
                    let price_id = normalize_opt!(record.price, prices);
                    let title_id = normalize_opt!(record.title, titles);
                    let main_cat_id = normalize_opt!(record.main_cat, main_cats);
                    let itemid = record.itemid as i32;
                    vec_main.push(RecordItem {
                        itemid,
                        brand_id,
                        description_id,
                        price_id,
                        title_id,
                        main_cat_id,
                    });
                }
                println!(
                        "{}, vec_record: {}, brands: {}, descriptions: {}, prices: {}, titles: {}, main_cats: {}",

                        arrange_millis::get(std::time::Instant::now().duration_since(start).as_millis()),
                        vec_main.len(), brands.len(), descriptions.len(), prices.len(), titles.len(), main_cats.len()
                        );
            }
        }

        {
            let start = std::time::Instant::now();
            let filepath = get_filepath(&data_dir, "itemid_asin.csv");
            println!("Read {:?}", filepath);
            let file = std::fs::File::open(&filepath).context(format!("{:?}", filepath))?;
            let mut vec_main = Vec::<RecordItemIdAsin>::new();
            let buffered_reader = BufReader::new(file);
            let mut rdr = csv::Reader::from_reader(buffered_reader);
            for result in rdr.deserialize() {
                let record: ItemIdAsin = result.context("ItemIdAsin")?;
                let itemid = record.itemid as i32;
                let asin = record.asin;
                vec_main.push(RecordItemIdAsin { itemid, asin })
            }
            println!(
                "{}, vec_main: {}",
                arrange_millis::get(std::time::Instant::now().duration_since(start).as_millis(),),
                vec_main.len(),
            );
        }

        {
            let filepath = get_filepath(&data_dir, "also_view_used.csv.zip");
            let contents = unzip(&filepath).context(format!("{:?}", filepath))?;
            {
                let start = std::time::Instant::now();
                let mut vec_main = Vec::<RecordAlsoView>::new();
                let buffered_reader = BufReader::new(contents.as_bytes());
                let mut rdr = csv::Reader::from_reader(buffered_reader);
                for result in rdr.deserialize() {
                    let record: AlsoViewUsed = result.context("AlsoViewUsed")?;
                    let itemid = record.itemid as i32;
                    let also_view_itemid = record.also_view_itemid as i32;
                    let is_train = record.is_train as i32;
                    vec_main.push(RecordAlsoView {
                        itemid,
                        also_view_itemid,
                        is_train,
                    })
                }
                println!(
                    "{}, vec_main: {}",
                    arrange_millis::get(
                        std::time::Instant::now().duration_since(start).as_millis(),
                    ),
                    vec_main.len(),
                );
            }
        }

        {
            let filepath = get_filepath(&data_dir, "category_used.csv.zip");
            let contents = unzip(&filepath).context(format!("{:?}", filepath))?;
            {
                let start = std::time::Instant::now();
                let mut dic_category = HashMap::<String, i32>::new();
                let mut vec_main = Vec::<RecordCategory>::new();
                let buffered_reader = BufReader::new(contents.as_bytes());
                let mut rdr = csv::Reader::from_reader(buffered_reader);
                for result in rdr.deserialize() {
                    let record: CategoryUsed = result.context("")?;
                    let itemid = record.itemid as i32;
                    let is_train = if record.is_train { 1 } else { 0 };
                    let category_id = normalize!(record.category, dic_category);
                    vec_main.push(RecordCategory {
                        itemid,
                        category_id,
                        is_train,
                    })
                }
                println!(
                    "{}, vec_main: {}, categorys: {}",
                    arrange_millis::get(
                        std::time::Instant::now().duration_since(start).as_millis(),
                    ),
                    vec_main.len(),
                    dic_category.len(),
                );
            }
        }
    }

    println!(
        "timing total: {}",
        arrange_millis::get(
            std::time::Instant::now()
                .duration_since(start_total)
                .as_millis(),
        ),
    );

    Ok(())
}

fn get_filepath(data_dir: &PathBuf, filename: &str) -> PathBuf {
    let mut filepath = data_dir.clone();
    filepath.push(filename);
    filepath
}

fn parse_style(style: &str, style_keys: &mut HashSet<String>, i: usize) -> Result<()> {
    lazy_static::lazy_static! {
        static ref RE_INCH: Regex = Regex::new(r#"(?P<prefix>[ /])(?P<inches>(:?(:?\d\.)?\d+)|Jar)""#).unwrap();
        static ref RE_INCH_REVERT: Regex = Regex::new(r#"<INCH>"#).unwrap();
        static ref RE_AND: Regex = Regex::new(r#" '[Nn]'? "#).unwrap();
        static ref RE_THEM: Regex = Regex::new(r#" 'Em "#).unwrap();
        static ref RE_THEM_REVERT: Regex = Regex::new(r#"<THEM>"#).unwrap();
        static ref RE_AND_REVERT: Regex = Regex::new(r#"<AND>"#).unwrap();
        static ref RE_QUOTE_TO_SKIP: Regex = Regex::new(r#"(?P<before>[^\s{])'(?P<after>[^},:])"#).unwrap();
        static ref RE_QUOTE_TO_SKIP_REVERT: Regex = Regex::new(r#"<QUOTE>"#).unwrap();
        //
        static ref RE_DOUBLEQUOTE_TO_SKIP: Regex = Regex::new(r#"(?P<before>[ ])"(?P<after>['])"#).unwrap();
        static ref RE_DOUBLEQUOTE_TO_SKIP_REVERT: Regex = Regex::new(r#"<DOUBLEQUOTE>"#).unwrap();
        static ref RE_DOUBLEQUOTED_TO_SKIP: Regex = Regex::new(r#""(?P<content>Bacon|Original)""#).unwrap();
        static ref RE_DOUBLEQUOTED_TO_SKIP_REVERT: Regex = Regex::new(r#"<DOUBLEQUOTED (?P<content>[^>]+)>"#).unwrap();
        static ref RE_QUOTE: Regex = Regex::new("'").unwrap();
    }
    let style_orig = style.to_owned();
    let style = RE_INCH.replace_all(&style, "$prefix$inches<INCHES>");
    let style = RE_AND.replace_all(&style, "<AND>").to_string();
    let style = RE_THEM.replace_all(&style, "<THEM>").to_string();
    let style = RE_QUOTE_TO_SKIP
        .replace_all(&style, "$before<QUOTE>$after")
        .to_string();
    let style = RE_DOUBLEQUOTE_TO_SKIP
        .replace_all(&style, "$before<DOUBLEQUOTE>$after")
        .to_string();
    let style = RE_DOUBLEQUOTED_TO_SKIP
        .replace_all(&style, "<DOUBLEQUOTED $content>")
        .to_string();
    let style = RE_QUOTE.replace_all(&style, "\"").to_string();
    let style = RE_AND_REVERT.replace_all(&style, " 'n' ").to_string();
    let style = RE_THEM_REVERT.replace_all(&style, " 'Em ").to_string();
    let style = RE_QUOTE_TO_SKIP_REVERT.replace_all(&style, "'");
    let style = RE_DOUBLEQUOTE_TO_SKIP_REVERT.replace_all(&style, "\\\"");
    let style = RE_DOUBLEQUOTED_TO_SKIP_REVERT.replace_all(&style, "\\\"$content\\\"");
    let style = RE_INCH_REVERT.replace_all(&style, "\"");
    let json = Json::from_str(&style, format!("[{}].style", i))
        .context(format!("orig: {}, from: {}", style_orig, style))?;

    for (key, _) in json.iter_map().context("json.iter_map()")? {
        style_keys.insert(key.to_owned());
    }
    Ok(())
}

fn unzip(filepath: &Path) -> Result<String> {
    let start = std::time::Instant::now();

    println!("unzip {:?} . . .", filepath);
    let file = File::open(&filepath).context("File::open(&filepath)")?;
    let buffered_reader = BufReader::new(file);
    let mut zip =
        zip::ZipArchive::new(buffered_reader).context("zip::ZipArchive::new(buffered_reader)")?;
    let mut file = zip.by_index(0).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let len = contents.len();
    println!(
        "{}, {}, unzip {:?}",
        arrange_millis::get(std::time::Instant::now().duration_since(start).as_millis()),
        if len < 1024 {
            format!("{}b", len)
        } else if len < 1024 * 1024 {
            format!("{}Kb", len / 1024)
        } else if len < 1024 * 1024 * 1024 {
            format!("{}Mb", len / (1024 * 1024))
        } else {
            format!("{}Gb", len / (1024 * 1024 * 1024))
        },
        filepath,
    );
    Ok(contents)
}
