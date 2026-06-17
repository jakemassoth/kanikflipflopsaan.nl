use serde::{Deserialize, Serialize};
use serde_json::json;
use vercel_runtime::{run, service_fn, Error, Request, Response, ResponseBody};

// De Bilt, Netherlands — the KNMI national reference station, roughly the centre of the country.
const LAT: f64 = 52.10;
const LON: f64 = 5.18;

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

async fn handler(req: Request) -> Result<Response<ResponseBody>, Error> {
    let persona = persona_from(&req);
    let forecast = fetch_forecast().await?;

    let factoren = evaluate(&forecast, persona.thresholds())?;
    let verdict = if factoren.iter().all(|f| f.ok) {
        "ja"
    } else {
        "nee"
    };

    let body = json!({
        "verdict": verdict,
        "persona": persona.id(),
        "factoren": factoren,
    })
    .to_string();

    // Align the Edge cache to the local day: it expires at the next midnight in
    // Amsterdam, so the daily report is recomputed once per calendar day. The
    // query string (persona) is part of the cache key, so each persona caches
    // independently.
    let ttl = seconds_until_local_midnight(forecast.current_time());

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .header("Cache-Control", format!("public, s-maxage={ttl}"))
        .body(ResponseBody::from(body))?)
}

// ---------------------------------------------------------------------------
// Personas — each has its own tolerance for the weather.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Persona {
    /// Boswachter — buitenmens, hoge tolerantie.
    Boswachter,
    /// Gewoon mens — de standaard.
    Gewoon,
    /// Koukleum — heeft het altijd koud, lage tolerantie.
    Koukleum,
}

struct Thresholds {
    min_temp_c: f64,
    max_precip_probability: f64, // %
    max_precip_mm: f64,          // fallback als er geen neerslagkans is
    max_wind_kmh: f64,
    max_gust_kmh: f64,
    max_weather_code: u8, // WMO code: hoger = natter/slechter weer
}

impl Persona {
    fn id(self) -> &'static str {
        match self {
            Persona::Boswachter => "boswachter",
            Persona::Gewoon => "gewoon",
            Persona::Koukleum => "koukleum",
        }
    }

    fn thresholds(self) -> Thresholds {
        match self {
            // Hoge tolerantie: het mag kouder, harder waaien en wat natter zijn.
            Persona::Boswachter => Thresholds {
                min_temp_c: 16.0,
                max_precip_probability: 80.0,
                max_precip_mm: 5.0,
                max_wind_kmh: 50.0,
                max_gust_kmh: 65.0,
                max_weather_code: 67, // tot en met regen; geen sneeuw/onweer
            },
            // De standaard.
            Persona::Gewoon => Thresholds {
                min_temp_c: 22.0,
                max_precip_probability: 50.0,
                max_precip_mm: 1.0,
                max_wind_kmh: 30.0,
                max_gust_kmh: 45.0,
                max_weather_code: 50, // codes >= 51 = motregen/regen/sneeuw/onweer
            },
            // Lage tolerantie: warmer, droger en windstiller voordat de slippers aan mogen.
            Persona::Koukleum => Thresholds {
                min_temp_c: 26.0,
                max_precip_probability: 20.0,
                max_precip_mm: 0.2,
                max_wind_kmh: 20.0,
                max_gust_kmh: 30.0,
                max_weather_code: 3, // alleen helder tot bewolkt
            },
        }
    }
}

/// Read the `persona` query parameter; unknown or missing falls back to `Gewoon`.
fn persona_from(req: &Request) -> Persona {
    let value = req
        .uri()
        .query()
        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("persona=")));

    match value {
        Some("boswachter") => Persona::Boswachter,
        Some("koukleum") => Persona::Koukleum,
        _ => Persona::Gewoon,
    }
}

// ---------------------------------------------------------------------------
// Open-Meteo (KNMI model) — today's daily forecast
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Forecast {
    current: Current,
    daily: Daily,
}

#[derive(Debug, Deserialize)]
struct Current {
    // Only used to know the local time (for the midnight cache alignment).
    time: String,
}

#[derive(Debug, Deserialize)]
struct Daily {
    weather_code: Vec<u8>,
    temperature_2m_max: Vec<f64>,
    // The KNMI deterministic model may not provide probability; treat as optional.
    precipitation_probability_max: Option<Vec<Option<f64>>>,
    precipitation_sum: Vec<f64>,
    wind_speed_10m_max: Vec<f64>,
    wind_gusts_10m_max: Vec<f64>,
}

impl Forecast {
    fn current_time(&self) -> &str {
        &self.current.time
    }
}

async fn fetch_forecast() -> Result<Forecast, Error> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast\
         ?latitude={LAT}&longitude={LON}\
         &models=knmi_seamless\
         &timezone=Europe%2FAmsterdam\
         &forecast_days=1\
         &current=temperature_2m\
         &daily=weather_code,temperature_2m_max,precipitation_probability_max,precipitation_sum,wind_speed_10m_max,wind_gusts_10m_max"
    );

    let forecast = reqwest::get(&url).await?.json::<Forecast>().await?;
    Ok(forecast)
}

// ---------------------------------------------------------------------------
// Decision logic — all factors must pass for "ja".
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Factor {
    label: String,
    detail: String,
    ok: bool,
}

fn evaluate(f: &Forecast, t: Thresholds) -> Result<Vec<Factor>, Error> {
    let d = &f.daily;
    let first = |v: &[f64]| v.first().copied().ok_or("lege dagvoorspelling");

    let temp_max = first(&d.temperature_2m_max)?;
    let wind_max = first(&d.wind_speed_10m_max)?;
    let gust_max = first(&d.wind_gusts_10m_max)?;
    let precip_sum = first(&d.precipitation_sum)?;
    let code = *d.weather_code.first().ok_or("lege dagvoorspelling")?;

    let mut factoren = Vec::new();

    // Temperatuur — dagmaximum moet boven de drempel uitkomen.
    factoren.push(Factor {
        label: "Temperatuur".to_string(),
        detail: format!("max {temp_max:.0}°C (moet > {:.0}°C)", t.min_temp_c),
        ok: temp_max > t.min_temp_c,
    });

    // Neerslag — gebruik de kans indien beschikbaar, anders de verwachte hoeveelheid.
    let prob = d
        .precipitation_probability_max
        .as_ref()
        .and_then(|v| v.first().copied().flatten());
    match prob {
        Some(p) => factoren.push(Factor {
            label: "Neerslagkans".to_string(),
            detail: format!("max {p:.0}% (moet < {:.0}%)", t.max_precip_probability),
            ok: p < t.max_precip_probability,
        }),
        None => factoren.push(Factor {
            label: "Neerslag".to_string(),
            detail: format!("{precip_sum:.1} mm (moet < {:.1} mm)", t.max_precip_mm),
            ok: precip_sum < t.max_precip_mm,
        }),
    }

    // Wind
    factoren.push(Factor {
        label: "Wind".to_string(),
        detail: format!(
            "max {wind_max:.0} km/u, stoten {gust_max:.0} km/u (max {:.0}/{:.0})",
            t.max_wind_kmh, t.max_gust_kmh
        ),
        ok: wind_max < t.max_wind_kmh && gust_max < t.max_gust_kmh,
    });

    // Weersgesteldheid
    factoren.push(Factor {
        label: "Weer".to_string(),
        detail: weer_omschrijving(code).to_string(),
        ok: code <= t.max_weather_code,
    });

    Ok(factoren)
}

/// WMO weather code → Dutch description.
fn weer_omschrijving(code: u8) -> &'static str {
    match code {
        0 => "helder",
        1 => "overwegend helder",
        2 => "half bewolkt",
        3 => "bewolkt",
        45 | 48 => "mist",
        51 | 53 | 55 => "motregen",
        56 | 57 => "ijzel (motregen)",
        61 | 63 | 65 => "regen",
        66 | 67 => "ijzel (regen)",
        71 | 73 | 75 => "sneeuw",
        77 => "sneeuwkorrels",
        80..=82 => "regenbuien",
        85 | 86 => "sneeuwbuien",
        95 => "onweer",
        96 | 99 => "onweer met hagel",
        _ => "onbekend",
    }
}

/// Seconds from the given local time (`YYYY-MM-DDTHH:MM`) until the next local
/// midnight. Used as the Edge cache TTL so the report refreshes once per day.
fn seconds_until_local_midnight(local_time: &str) -> u64 {
    let hh: u64 = local_time
        .get(11..13)
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let mm: u64 = local_time
        .get(14..16)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let elapsed = hh * 3600 + mm * 60;
    // At least 60s so a request right before midnight still caches briefly.
    86_400u64.saturating_sub(elapsed).max(60)
}
