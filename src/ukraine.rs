use crate::{alerts::*, constants::*};
#[allow(unused)]
use anyhow::*;
use arrayvec::ArrayVec;
use delegate::delegate;
#[allow(unused)]
use either::Either;
use geo::{Coord, Polygon};
use getset::{Getters, MutGetters, Setters};
use ratatui::{
    layout::Rect,
    prelude::*,
    widgets::canvas::{Painter, Shape},
    widgets::{ListItem, ListState},
};
use serde::*;
use tracing::info;

// use geo::algorithm::bounding_rect::BoundingRect;
// use geo::algorithm::simplify_vw::SimplifyVw;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Region {
    pub id: i8,
    pub a_id: i8,
    pub osm_id: i64,
    pub geo: String,
    pub name: String,
    pub name_en: String,
    #[sqlx(rename = "status", default)]
    pub status: Option<String>,
}

pub type RegionArrayVec = ArrayVec<Region, 27>;
pub type RegionListVec<'a> = ArrayVec<ListItem<'a>, 27>;

impl Region {
    pub fn to_list_item(&self, index: i8, alert_status: char) -> ListItem<'static> {
        let name = self.name.clone();
        // let bg_color = match index % 2 { 0 => NORMAL_ROW_COLOR, _ => ALERT_ROW_COLOR, };
        let line = match alert_status {
            'A' => Line::styled(format!("{}) {} ⊙", index, name), ALERT_ROW_COLOR),
            'P' => Line::styled(format!("{}) {}", index, name), MARKER_COLOR),
            _ => Line::styled(format!("{}) {}", index, name), TEXT_COLOR),
        };

        ListItem::new(line)
    }
}

#[derive(Debug, Default, Clone, Getters, MutGetters, Setters)]
struct RegionsList {
    #[getset(get = "pub", get_mut = "pub", set = "pub")]
    items: Vec<ListItem<'static>>,
    #[getset(get = "pub", get_mut = "pub")]
    state: ListState,
    #[getset(get = "pub", get_mut = "pub")]
    last_selected: Option<usize>,
}

impl RegionsList {
    pub fn new(
        regions: &[Region],
        alerts_statuses: &[char],
        // alertss: Chars<'static>,
    ) -> Self {
        // let iter = alerts_string.chars();
        let items: Vec<ListItem> = regions
            .iter()
            .enumerate()
            .map(|(i, r)| r.to_list_item(i as i8, alerts_statuses[i]))
            .collect();
        let state = ListState::default();
        let last_selected = None;
        Self {
            items,
            state,
            last_selected,
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
        info!("List->next, selected region: {:?}", i);
    }

    #[tracing::instrument(skip(self))]
    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
        info!("List->previous, selected region: {:?}", i);
    }

    pub fn unselect(&mut self) {
        let offset = self.state.offset();
        self.last_selected = self.state.selected();
        self.state.select(None);
        *self.state.offset_mut() = offset;
    }

    pub fn go_top(&mut self) {
        self.state.select(Some(0));
    }

    pub fn go_bottom(&mut self) {
        self.state.select(Some(self.items.len() - 1));
    }
}

#[derive(Debug, Default, Getters, Setters)]
pub struct Ukraine {
    borders: String,
    #[getset(get = "pub")]
    regions: RegionArrayVec,
    pub center: Coord,
    #[getset(get = "pub", set = "pub")]
    size: Rect,
    #[getset(get = "pub", set = "pub")]
    list: RegionsList,
}

impl Ukraine {
    pub fn new(
        borders: String,
        regions: RegionArrayVec,
        alerts_string: AlertsResponseString,
    ) -> Self {
        let center = Coord::from(CENTER);
        let bbox = Rect::default();
        let alerts_statuses: Vec<char> = alerts_string.chars().collect::<Vec<char>>();
        let list = RegionsList::new(
            regions.as_slice(),
            alerts_statuses.as_slice(),
        );

        Self {
            borders,
            regions,
            center,
            size: bbox,
            list,
        }
    }

    delegate! {
        to self.list {
            #[call(items)]
            pub fn get_list_items(&mut self) -> &Vec<ListItem<'static>>;
            #[call(set_items)]
            pub fn set_list_items(&mut self, items: Vec<ListItem<'static>>);
            pub fn next(&mut self);
            pub fn previous(&mut self);
            pub fn unselect(&mut self);
            pub fn go_top(&mut self);
            pub fn go_bottom(&mut self);
        }

        to self.list.items {
            #[call(len)]
            pub fn list_size(&self) -> usize;
            // pub fn items(&self) -> &Vec<Region>;
        }
    }

    pub fn borders(&self) -> Polygon {
        use std::str::FromStr;
        use wkt::Wkt;
        let geom: Polygon = Wkt::from_str(&self.borders).unwrap().try_into().unwrap();
        geom
    }

    pub fn list_state(&self) -> &ListState {
        self.list.state()
    }

    #[inline]
    pub fn boundingbox(&self) -> [(f64, f64); 2] {
        #[allow(unused_parens)]
        (BOUNDINGBOX)
    }

    #[inline]
    pub fn x_bounds(&self) -> [f64; 2] {
        [
            self.boundingbox().first().unwrap().0 - PADDING,
            self.boundingbox().last().unwrap().0 + PADDING,
        ]
    }

    #[inline]
    pub fn y_bounds(&self) -> [f64; 2] {
        [
            self.boundingbox().first().unwrap().1 - PADDING,
            self.boundingbox().last().unwrap().1 + PADDING,
        ]
    }

    /// Store size of the terminal rect
    #[inline]
    pub fn set_map_size(&mut self, rect: Rect) {
        info!("Ukraine->set_size: {:?}", rect);
        self.size = rect;
    }
    /// Get the resolution of the grid in number of dots
    #[inline]
    pub fn resolution(&self) -> (f64, f64) {
        (
            f64::from(self.size.width) * 2.0,
            f64::from(self.size.height) * 4.0,
        )
    }

    /// update list items with alerts and change item status
    pub fn set_alerts(&mut self, alerts: Vec<Alert>) {
        info!("Ukraine->set_alerts: {:?}", alerts);
        let mut regions = ArrayVec::<Region, 27>::new();
        self.regions.iter_mut().for_each(|item| {
            if let Some(alert) = alerts
                .iter()
                .find(|alert| alert.location_oblast_uid.unwrap() == item.id as i32)
            {
                if Some(alert).is_some() {
                    item.status = Some("A".to_string());
                }
            } else {
                item.status = None;
            }
            regions.push(item.clone());
        });

        self.regions = regions
    }
}

impl Shape for Ukraine {
    /// Implement the Shape trait for Ukraine to draw map borders on canvas
    #[tracing::instrument]
    #[inline]
    fn draw(&self, painter: &mut Painter) {
        let borders = self.borders();
        let coords_iter = borders.exterior().coords().into_iter();
        coords_iter.for_each(|&coord| {
            if let Some((x, y)) = painter.get_point(coord.x, coord.y) {
                painter.paint(x, y, MARKER_COLOR);
            }
        });
        // TODO: mark center - not working
        if let Some((cx, cy)) = painter.get_point(self.center.x, self.center.y) {
            painter.paint(cx, cy, MARKER_COLOR);
        }
    }
}
