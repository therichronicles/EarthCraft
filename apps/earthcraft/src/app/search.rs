use super::App;
use earthcraft_core::coordinates::parse_latlon;
use earthcraft_core::language::{Lang, fold_name};
use earthcraft_data::search::merge_suggestions;
use earthcraft_data::search::{geocode, photon_suggest};
use eframe::egui;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
const SUGGESTION_DEBOUNCE_DELAY: Duration = Duration::from_millis(280);
const SUGGESTION_POLL_INTERVAL: Duration = Duration::from_millis(90);

impl App {
    pub(super) fn on_query_changed(&mut self) {
        let q = self.query.trim().to_string();
        // Results and their status belong to the query that produced them.
        // Dropping its receiver also prevents a late response restoring old results.
        self.search_rx = None;
        self.results.clear();
        self.status = self.lang.text("app-ready-status").into();
        if q.is_empty() {
            self.sugg_rx = None;
            self.sugg_sent.clear();
        }
        self.sugg_sel = None;
        self.sugg_last_edit = Instant::now();
        if q.is_empty() || parse_latlon(&q).is_some() {
            self.sugg_open = false;
            self.suggestions.clear();
            self.remote_sugg.clear();
            return;
        }
        self.sugg_open = true;
        let fq = fold_name(&q);
        self.remote_sugg
            .retain(|s| fold_name(&s.label).contains(&fq));
        self.rebuild_suggestions();
    }

    /// Instant local matches (countries, states, cities already loaded) + remote results.
    pub(super) fn rebuild_suggestions(&mut self) {
        self.suggestions = self
            .local_search
            .merge(&self.query, self.lang, &self.remote_sugg)
            .unwrap_or_else(|| {
                merge_suggestions(
                    &self.query,
                    self.lang,
                    &self.boundaries.labels,
                    &self.remote_sugg,
                )
            });
        if self.sugg_sel.is_some_and(|i| i >= self.suggestions.len()) {
            self.sugg_sel = None;
        }
    }

    pub(super) fn poll_suggestions(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.sugg_rx
            && let Ok((q, res)) = rx.try_recv()
        {
            self.sugg_rx = None;
            if let Ok(v) = res
                && q == self.query.trim()
            {
                self.remote_sugg = v;
                self.rebuild_suggestions();
            }
            ctx.request_repaint();
        }
        let q = self.query.trim().to_string();
        if self.sugg_open && q.chars().count() >= 2 && q != self.sugg_sent && self.sugg_rx.is_none()
        {
            if self.sugg_last_edit.elapsed() >= SUGGESTION_DEBOUNCE_DELAY {
                self.sugg_sent = q.clone();
                let (tx, rx) = mpsc::channel();
                self.sugg_rx = Some(rx);
                let (ctx2, lang) = (ctx.clone(), self.lang.code());
                self.runtime.spawn(async move {
                    let r = photon_suggest(&q, lang).await;
                    let _ = tx.send((q, r));
                    ctx2.request_repaint();
                });
            } else {
                ctx.request_repaint_after(SUGGESTION_POLL_INTERVAL);
            }
        }
    }

    pub(super) fn choose_suggestion(&mut self, i: usize) {
        if let Some(s) = self.suggestions.get(i).cloned() {
            self.search_rx = None;
            self.query = s.label.clone();
            self.sugg_sent = s.label.clone();
            self.sugg_open = false;
            self.sugg_sel = None;
            self.results.clear();
            self.fly_to(s.lat, s.lon, s.alt);
            self.status = self.lang.format(
                "search-navigation-status",
                &[("destination", s.label.into())],
            );
        }
    }

    pub(super) fn poll_search(&mut self) {
        let lg = self.lang;
        let Some(rx) = &self.search_rx else { return };
        if let Ok(res) = rx.try_recv() {
            self.search_rx = None;
            match res {
                Ok(v) if v.is_empty() => self.status = lg.text("search-empty-status").into(),
                Ok(v) => {
                    self.status = lg.format("search-results-status", &[("count", v.len().into())]);
                    let first = v[0].clone();
                    self.results = v;
                    self.fly_to(first.lat, first.lon, first.alt);
                }
                Err(e) => {
                    self.status =
                        Lang::En.format("search-request-error", &[("error", e.to_string().into())])
                }
            }
        }
    }

    pub(super) fn start_search(&mut self, ctx: &egui::Context) {
        let q = self.query.trim().to_string();
        if q.is_empty() {
            return;
        }
        // Secret easter egg commands are local-only (no network request).
        if matches!(fold_name(&q).as_str(), "terraplana" | "flatearth") {
            self.unlock_flat_earth(ctx);
            return;
        }
        if let Some((lat, lon)) = parse_latlon(&q) {
            self.fly_to(lat, lon, 3.0);
            self.status = self.lang.format(
                "search-navigation-status",
                &[("destination", format!("{lat:.5}, {lon:.5}").into())],
            );
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.search_rx = Some(rx);
        self.status = self.lang.text("search-pending-status").into();
        let ctx = ctx.clone();
        let lang_code = self.lang.code();
        self.runtime.spawn(async move {
            let _ = tx.send(geocode(&q, lang_code).await);
            ctx.request_repaint();
        });
    }
}
