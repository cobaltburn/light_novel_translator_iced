use crate::{
    model::Activity,
    widget::{active_mark, check_mark, context_menu_button, cross_mark, text_button},
};
use iced::{
    Color, Element, Length, Padding,
    widget::{Column, column, container, row, scrollable, text},
};
use iced_aw::ContextMenu;

pub trait SidebarAction: Clone + 'static {
    fn set_page(page: usize) -> Self;
    fn save_page(name: String, page: usize) -> Self;
    fn translate(page: usize) -> Self;
    fn translate_page(page: usize) -> Self;
    fn translate_part(page: usize, part: usize) -> Self;
}

#[derive(Hash)]
pub struct SidebarDeps {
    pub current_page: usize,
    pub active: bool,
    pub rows: Vec<SidebarRow>,
}

#[derive(Hash)]
pub struct SidebarRow {
    pub name: String,
    pub activity: Activity,
    pub section_count: usize,
}

pub fn build_path_buttons<A: SidebarAction>(deps: &SidebarDeps) -> Column<'static, A> {
    let current = deps.current_page;
    let active = deps.active;

    deps.rows
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let name = entry.name.clone();
            let activity = entry.activity.clone();
            let section_count = entry.section_count;

            let button_text =
                text!("{}. {}", i + 1, &name)
                    .width(Length::Fill)
                    .style(move |theme| {
                        if current == i {
                            text::primary(theme)
                        } else {
                            text::default(theme)
                        }
                    });

            let button_content = row![button_text]
                .push(match activity {
                    Activity::Incomplete => None,
                    Activity::Complete => Some(check_mark()),
                    Activity::Error(e) => Some(row![text(e), cross_mark()].spacing(5).into()),
                    Activity::Active => Some(active_mark()),
                })
                .padding(Padding::default().right(10));

            ContextMenu::new(
                text_button(button_content).on_press(A::set_page(i)),
                move || path_button_overlay::<A>(section_count, name.clone(), i, active),
            )
            .into()
        })
        .collect()
}

fn path_button_overlay<A: SidebarAction>(
    count: usize,
    name: String,
    page: usize,
    active: bool,
) -> Element<'static, A> {
    let overlay = column![
        context_menu_button(text("save").color(Color::WHITE)).on_press(A::save_page(name, page)),
        context_menu_button(text("translate").color(Color::WHITE))
            .on_press_maybe(active.then(|| A::translate(page))),
        context_menu_button(text("translate page").color(Color::WHITE))
            .on_press_maybe(active.then(|| A::translate_page(page)))
    ]
    .extend((0..count).map(|part| {
        context_menu_button(text!("translate part {}", part + 1).color(Color::WHITE))
            .on_press_maybe(active.then(|| A::translate_part(page, part)))
            .into()
    }))
    .padding(5)
    .spacing(5);

    container(scrollable(overlay).width(Length::Fill))
        .style(container::rounded_box)
        .max_height(400)
        .width(300)
        .into()
}