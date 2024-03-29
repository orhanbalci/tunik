use std::rc::Rc;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use tui::{
    buffer::Buffer,
    layout::{Corner, Rect},
    style::{Color, Style},
    text::{Span, Spans, Text},
    widgets::{Block, StatefulWidget, Widget},
};
use unicode_width::UnicodeWidthStr;

#[derive(Clone)]
pub struct FuzzyListState<'a> {
    offset: usize,
    selected: Option<usize>,
    filter: Option<String>,
    items: Rc<Vec<FuzzyListItem<'a>>>,
    filtered: Rc<Vec<FuzzyListItem<'a>>>,
    /// matcher algorithm
    matcher: Rc<dyn FuzzyMatcher>,
}

impl<'a> Default for FuzzyListState<'a> {
    fn default() -> Self {
        FuzzyListState {
            offset: 0,
            selected: None,
            filter: None,
            items: Rc::new(vec![]),
            filtered: Rc::new(vec![]),
            matcher: Rc::new(SkimMatcherV2::default()),
        }
    }
}

impl<'a> FuzzyListState<'a> {
    pub fn with_items(items: Vec<FuzzyListItem<'a>>) -> Self {
        FuzzyListState {
            offset: 0,
            selected: None,
            filter: None,
            items: Rc::new(items),
            filtered: Rc::new(vec![]),
            matcher: Rc::new(SkimMatcherV2::default()),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    pub fn increment_selected(&mut self) {
        self.select(self.selected.map(|v| v + 1).or(Some(0)));
    }

    pub fn decrement_selected(&mut self) {
        self.select(self.selected.map(|v| if v > 0 { v - 1 } else { v }));
    }

    pub fn get_filter(&self) -> Option<String> {
        self.filter.clone()
    }

    pub fn set_filter(&mut self, filter: Option<&str>) {
        let filter = filter.filter(|f| !f.is_empty());
        let should_filter = match (filter, self.filter.clone()) {
            (None, Some(_)) => {
                self.filtered = Rc::new(vec![]);
                false
            }
            (Some(_), None) => true,
            (Some(x), Some(y)) if *x != y => true,
            (None, None) => false,
            _ => false,
        };
        if should_filter {
            let len = self.items.len();
            self.filtered = Rc::new(
                (0..len)
                    .map(|i| self.items[i].clone())
                    .filter_map(|mut item| {
                        if item.matches(&self.matcher, filter.unwrap()) {
                            Some(item.clone())
                        } else {
                            None
                        }
                    })
                    .collect(),
            );
            self.selected = None;
        }
        self.filter = filter
            .map(|f| f.into())
            .and_then(|f: String| if f.is_empty() { None } else { Some(f) });
    }

    pub fn get_items(&self) -> Rc<Vec<FuzzyListItem<'a>>> {
        if self.filtered.is_empty() {
            self.items.clone()
        } else {
            self.filtered.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyListItem<'a> {
    content: Text<'a>,
    style: Style,
    filter_style: Style,
}

impl<'a> FuzzyListItem<'a> {
    pub fn new<T>(content: T) -> FuzzyListItem<'a>
    where
        T: Into<Text<'a>>,
    {
        FuzzyListItem {
            content: content.into(),
            style: Style::default(),
            filter_style: Style::default().fg(Color::Red),
        }
    }

    pub fn style(mut self, style: Style) -> FuzzyListItem<'a> {
        self.style = style;
        self
    }

    pub fn filter_style(mut self, filter_style: Style) -> FuzzyListItem<'a> {
        self.filter_style = filter_style;
        self
    }

    pub fn height(&self) -> usize {
        self.content.height()
    }

    pub fn matches(&mut self, matcher: &Rc<dyn FuzzyMatcher>, filter: &str) -> bool {
        let mut matches = false;
        self.content.lines.iter_mut().for_each(|spans| {
            let spans_cloned = spans.clone();
            let filtered_spans: Vec<Span> = spans_cloned
                .0
                .iter()
                .flat_map(|span| {
                    let content = span.content.as_ref();
                    let match_indices = matcher.fuzzy_indices(content, filter);
                    if let Some(indices) = match_indices {
                        matches = true;
                        // dbg!(&indices);
                        let index = *indices.1.first().unwrap();

                        // consider only first match. split text into three or two partes
                        if index > 0 && index < content.len() - filter.len() {
                            vec![
                                Span::raw(String::from(&content[0..index])),
                                Span::styled(
                                    String::from(&content[index..index + filter.len()]),
                                    self.filter_style,
                                ),
                                Span::raw(String::from(&content[index + filter.len()..])),
                            ]
                        } else if index == 0 {
                            vec![
                                Span::styled(
                                    String::from(&content[0..filter.len()]),
                                    self.filter_style,
                                ),
                                Span::raw(String::from(&content[filter.len()..])),
                            ]
                        } else {
                            vec![
                                Span::raw(String::from(&content[0..content.len() - filter.len()])),
                                Span::styled(
                                    String::from(&content[content.len() - filter.len()..]),
                                    self.filter_style,
                                ),
                            ]
                        }
                    } else {
                        vec![Span::raw(String::from(content))]
                    }
                })
                .collect();
            *spans = Spans::from(filtered_spans);
        });
        matches
    }
}

/// A widget to display several items among which one can be selected (optional)
/// Supports fuzzy filtering of content
/// # Examples
///
/// ```
/// # use tui::widgets::{Block, Borders, List, ListItem};
/// # use tui::style::{Style, Color, Modifier};
/// let items = [ListItem::new("Item 1"), ListItem::new("Item 2"), ListItem::new("Item 3")];
/// List::new(items)
///     .block(Block::default().title("List").borders(Borders::ALL))
///     .style(Style::default().fg(Color::White))
///     .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
///     .highlight_symbol(">>");
/// ```
#[derive(Clone)]
pub struct FuzzyList<'a> {
    block: Option<Block<'a>>,
    items: Rc<Vec<FuzzyListItem<'a>>>,
    /// Style used as a base style for the widget
    style: Style,
    start_corner: Corner,
    /// Style used to render selected item
    highlight_style: Style,
    /// Symbol in front of the selected item (Shift all items to the right)
    highlight_symbol: Option<&'a str>,
    /// Whether to repeat the highlight symbol for each line of the selected item
    repeat_highlight_symbol: bool,
}

impl<'a> FuzzyList<'a> {
    pub fn new(items: Rc<Vec<FuzzyListItem<'a>>>) -> FuzzyList<'a> {
        FuzzyList {
            block: None,
            style: Style::default(),
            items,
            start_corner: Corner::TopLeft,
            highlight_style: Style::default(),
            highlight_symbol: None,
            repeat_highlight_symbol: false,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> FuzzyList<'a> {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> FuzzyList<'a> {
        self.style = style;
        self
    }

    pub fn highlight_symbol(mut self, highlight_symbol: &'a str) -> FuzzyList<'a> {
        self.highlight_symbol = Some(highlight_symbol);
        self
    }

    pub fn highlight_style(mut self, style: Style) -> FuzzyList<'a> {
        self.highlight_style = style;
        self
    }

    pub fn repeat_highlight_symbol(mut self, repeat: bool) -> FuzzyList<'a> {
        self.repeat_highlight_symbol = repeat;
        self
    }

    pub fn start_corner(mut self, corner: Corner) -> FuzzyList<'a> {
        self.start_corner = corner;
        self
    }

    fn get_items_bounds(
        &self,
        selected: Option<usize>,
        offset: usize,
        max_height: usize,
    ) -> (usize, usize) {
        let offset = offset.min(self.items.len().saturating_sub(1));
        let mut start = offset;
        let mut end = offset;
        let mut height = 0;
        for item in self.items.iter().skip(offset) {
            if height + item.height() > max_height {
                break;
            }
            height += item.height();
            end += 1;
        }

        let selected = selected.unwrap_or(0).min(self.items.len() - 1);

        while selected >= end {
            height = height.saturating_add(self.items[end].height());
            end += 1;
            while height > max_height {
                height = height.saturating_sub(self.items[start].height());
                start += 1;
            }
        }
        while selected < start {
            start -= 1;
            height = height.saturating_add(self.items[start].height());
            while height > max_height {
                end -= 1;
                height = height.saturating_sub(self.items[end].height());
            }
        }
        (start, end)
    }
}

impl<'a> StatefulWidget for FuzzyList<'a> {
    type State = FuzzyListState<'a>;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        buf.set_style(area, self.style);
        let list_area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        if list_area.width < 1 || list_area.height < 1 {
            return;
        }

        if self.items.is_empty() {
            return;
        }

        let list_height = list_area.height as usize;

        let (start, end) = self.get_items_bounds(state.selected, state.offset, list_height);
        state.offset = start;

        let highlight_symbol = self.highlight_symbol.unwrap_or("");
        let blank_symbol = " ".repeat(highlight_symbol.width());

        let mut current_height = 0;
        let has_selection = state.selected.is_some();
        for (i, item) in self
            .items
            .iter()
            .enumerate()
            .skip(state.offset)
            .take(end - start)
        {
            let (x, y) = match self.start_corner {
                Corner::BottomLeft => {
                    current_height += item.height() as u16;
                    (list_area.left(), list_area.bottom() - current_height)
                }
                _ => {
                    let pos = (list_area.left(), list_area.top() + current_height);
                    current_height += item.height() as u16;
                    pos
                }
            };
            let area = Rect {
                x,
                y,
                width: list_area.width,
                height: item.height() as u16,
            };
            let item_style = self.style.patch(item.style);
            buf.set_style(area, item_style);

            let is_selected = state.selected.map(|s| s == i).unwrap_or(false);
            for (j, line) in item.content.lines.iter().enumerate() {
                // if the item is selected, we need to display the hightlight symbol:
                // - either for the first line of the item only,
                // - or for each line of the item if the appropriate option is set
                let symbol = if is_selected && (j == 0 || self.repeat_highlight_symbol) {
                    highlight_symbol
                } else {
                    &blank_symbol
                };
                let (elem_x, max_element_width) = if has_selection {
                    let (elem_x, _) = buf.set_stringn(
                        x,
                        y + j as u16,
                        symbol,
                        list_area.width as usize,
                        item_style,
                    );
                    (elem_x, (list_area.width - (elem_x - x)))
                } else {
                    (x, list_area.width)
                };
                buf.set_spans(elem_x, y + j as u16, line, max_element_width);
            }
            if is_selected {
                buf.set_style(area, self.highlight_style);
            }
        }
    }
}

impl<'a> Widget for FuzzyList<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = FuzzyListState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
