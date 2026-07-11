use cinewana_core::CatalogQuery;
use cinewana_database::Database;

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).ok_or_else(|| anyhow::anyhow!("usage: library_summary <database-path>"))?;
    let db = Database::open(path)?;
    let items = db.catalog(&CatalogQuery::default())?;
    let movies = items.iter().filter(|m| matches!(m.kind, cinewana_core::MediaKind::Movie)).count();
    let episodes = items.len() - movies;
    let series = db.home()?.series.len();
    println!("files={} movies={} episodes={} series={}", items.len(), movies, episodes, series);
    for item in items {
        println!("{:?}\t{}\tS{:02?}E{:02?}", item.kind, item.series_title.as_deref().unwrap_or(&item.title), item.season_number, item.episode_number);
    }
    Ok(())
}

