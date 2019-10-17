#![deny(warnings)]

extern crate josh;

extern crate rs_tracing;

extern crate clap;
extern crate git2;
extern crate regex;

#[macro_use]
extern crate lazy_static;

use josh::view_maps;
use std::env;
use std::process::exit;

use std::fs::read_to_string;

lazy_static! {
    static ref FILE_REGEX: regex::Regex =
        regex::Regex::new(r"\[(?P<src>.*)\](?P<spec>[^\[]*)").expect("can't compile regex");
    static ref STR_REGEX: regex::Regex =
        regex::Regex::new(r"(?P<src>[^:]*)(?P<spec>:[^\[]*)").expect("can't compile regex");
}

fn run_filter(args: Vec<String>) -> i32 {
    let args = clap::App::new("josh-filter")
        .arg(clap::Arg::with_name("file").long("file").takes_value(true))
        .arg(clap::Arg::with_name("from_to").takes_value(true))
        .arg(clap::Arg::with_name("spec").takes_value(true))
        .arg(clap::Arg::with_name("squash").long("squash"))
        .arg(clap::Arg::with_name("reverse").long("reverse"))
        .arg(clap::Arg::with_name("infofile").long("infofile"))
        .arg(
            clap::Arg::with_name("trace")
                .long("trace")
                .takes_value(true),
        )
        .get_matches_from(args);

    let repo = git2::Repository::open_from_env().unwrap();
    let mut fm = view_maps::ViewMaps::new();
    let mut bm = view_maps::ViewMaps::new();

    let srcstr = args.value_of("from_to").unwrap_or("");
    let specstr = args.value_of("spec").unwrap_or("");

    let filestr = args
        .value_of("file")
        .and_then(|f| read_to_string(f).ok())
        .unwrap_or(format!("[{}]{}", srcstr, specstr));

    for caps in FILE_REGEX.captures_iter(&filestr) {
        let from_to = caps.name("src").unwrap().as_str().trim().to_owned();
        let mut splitted = from_to.splitn(2, ":");

        let src = splitted
            .next()
            .expect("from_to must contain \":\"")
            .to_owned();
        let target = splitted
            .next()
            .expect("from_to must contain \":\"")
            .to_owned();

        let viewstr = caps.name("spec").unwrap().as_str().trim().to_owned();

        let mut viewobj = josh::build_view(&repo, &viewstr);

        let pres = viewobj.prefixes();

        if args.is_present("infofile") {
            for (p, v) in pres.iter() {
                viewobj = josh::build_chain(
                    viewobj,
                    josh::build_view(
                        &repo,
                        &format!(
                            ":info={},commit=#sha1,tree=#tree,src={},view={}",
                            p,
                            &src,
                            v.replace(":", "<colon>").replace(",", "<comma>")
                        ),
                    ),
                );
            }
        }

        let reverse = args.is_present("reverse");

        if args.is_present("squash") {
            viewobj = josh::build_chain(
                josh::build_view(&repo, &format!(":cutoff={}", &src)),
                viewobj,
            );
        }

        let t = if reverse {
            "refs/JOSH_TMP".to_owned()
        } else {
            target.clone()
        };
        let src = repo
            .revparse_ext(&src)
            .expect("reference not found 1")
            .1
            .expect("reference not found")
            .name()
            .unwrap()
            .to_string();

        josh::apply_view_to_refs(&repo, &*viewobj, &[(src.clone(), t)], &mut fm, &mut bm);

        if reverse {
            let new = repo.revparse_single(&target).unwrap().id();
            let old = repo.revparse_single("JOSH_TMP").unwrap().id();

            match josh::unapply_view(&repo, &bm, &*viewobj, old, new) {
                josh::UnapplyView::Done(rewritten) => {
                    repo.reference(&src, rewritten, true, "unapply_view")
                        .expect("can't create reference");
                }
                _ => {
                    /* debug!("rewritten ERROR"); */
                    return 1;
                }
            }
        }
    }

    return 0;
}

fn main() {
    let args = {
        let mut args = vec![];
        for arg in env::args() {
            args.push(arg);
        }
        args
    };

    exit(run_filter(args));
}
