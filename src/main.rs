use iced::{
    border, color, event, keyboard, padding,
    theme::Palette,
    widget::{
        button, column, container,
        markdown::{self, Highlight},
        row, scrollable, stack, text,
    },
    Alignment, Background, Border, Element, Event, Length, Padding, Shadow, Subscription, Task,
    Theme,
};
use serde::Deserialize;
use std::{
    cmp::min,
    collections::BTreeMap,
    error::Error,
    fs::File,
    io::{BufReader, Read},
    path::Path,
    process,
};

fn custom_theme() -> Theme {
    Theme::custom(
        "CustomLatte".to_string(),
        Palette {
            background: color!(0x2E, 0x34, 0x40),
            text: color!(0xD8, 0xDE, 0xE9),
            primary: color!(0x81, 0xA1, 0xC1),
            success: color!(0xA3, 0xBE, 0x8C),
            danger: color!(0xBF, 0x61, 0x6A),
        },
    )
}

pub fn main() -> iced::Result {
    iced::application("Win tool box", WinToolBox::update, WinToolBox::view)
        .subscription(WinToolBox::subscription)
        .theme(|_| custom_theme())
        .antialiasing(true)
        .centered()
        .decorations(false)
        .run_with(WinToolBox::new)
}

#[derive(Debug, Default)]
pub enum StatusMessageType {
    Error,
    Success,
    #[default]
    Info,
}

#[derive(Default)]
struct WinToolBox {
    current_programm_markdown: Vec<markdown::Item>,
    current_programm: Option<Programm>,
    programms: BTreeMap<String, Programm>,
    config_name: String,
    status_message: (String, StatusMessageType),
    cur_menu: ControlMenuVariations,
    help_md: Vec<markdown::Item>,
    search_text: String,
    search_selected: bool,
    selected_result_index: usize,
    search_programms_names: Vec<String>,
}

#[derive(Default, Deserialize, Clone)]
pub struct Programm {
    name: String,
    call: Option<String>,
    description_md: String,
    docs_link: Option<String>,
    status: ProgrammStatus,
    installation: String,
    deletion: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ProgrammManipulation {
    Install,
    Uninstall,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ControlMenuVariations {
    HelpMenu,
    #[default]
    ProgrammsMenu,
    ConfigsMenu,
    ExitProgramm,
}

#[derive(Debug, Clone)]
enum Message {
    SelectProgrammFromList(String),
    RunProgrammDefault,
    DescriptionAndDocsLinkClicked(markdown::Url),
    OpenContainingFolder,
    OpenDocsOnline,
    ControlMenuBtn(ControlMenuVariations),
    ManipulateProgramm(ProgrammManipulation),
    Manipulationresult(ProgrammManipulation, Result<(), String>),
    AppEvent(Event),
}

impl WinToolBox {
    fn new() -> (Self, Task<Message>) {
        let (progs, conf_name) = load_config("programms.json").expect("Can't load a config!");
        (
            WinToolBox {
                current_programm_markdown: Vec::new(),
                programms: progs,
                current_programm: None,
                config_name: conf_name,
                status_message: ("Ok!".to_string(), StatusMessageType::Success),
                cur_menu: ControlMenuVariations::ProgrammsMenu,
                help_md: Vec::new(),
                search_text: String::new(),
                search_selected: false,
                selected_result_index: 0,
                search_programms_names: Vec::new(),
            },
            Task::none(),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen().map(Message::AppEvent)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DescriptionAndDocsLinkClicked(link) => {
                if let Err(e) = opener::open(link.as_str()) {
                    self.status_message =
                        (format!("Can't open link: {}", e), StatusMessageType::Error);
                }
                Task::none()
            }
            Message::OpenDocsOnline => {
                if let Some(cur_programm) = &self.current_programm {
                    if let Some(docs_link) = &cur_programm.docs_link {
                        if let Err(e) = opener::open(docs_link) {
                            self.status_message =
                                (format!("Can't open link: {}", e), StatusMessageType::Error);
                        }
                    }
                }
                Task::none()
            }
            Message::OpenContainingFolder => {
                if let Some(cur_programm) = &self.current_programm {
                    if let Some(call) = &cur_programm.call {
                        let output = process::Command::new("where").arg(call).output();
                        match output {
                            Ok(output) if output.status.success() => {
                                let path_str = String::from_utf8(output.stdout)
                                    .unwrap_or_default()
                                    .trim()
                                    .to_string();
                                let first_path = path_str.lines().next().unwrap_or("");
                                let folder_path =
                                    Path::new(first_path).parent().unwrap_or(Path::new(""));
                                if let Err(e) =
                                    process::Command::new("explorer").arg(folder_path).spawn()
                                {
                                    self.status_message = (
                                        format!("Can't open explorer: {}", e),
                                        StatusMessageType::Error,
                                    );
                                }
                            }
                            _ => {
                                self.status_message = (
                                    format!("Program not found: {:#?}", output),
                                    StatusMessageType::Error,
                                )
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::SelectProgrammFromList(select_prog_name) => {
                self.search_selected = false;
                self.selected_result_index = 0;
                for (prog_name, prog) in &self.programms {
                    if *prog_name == select_prog_name {
                        self.current_programm_markdown =
                            markdown::parse(&prog.description_md).collect();
                        self.current_programm = Some(prog.clone());
                    }
                }
                Task::none()
            }
            Message::RunProgrammDefault => {
                if let Some(cur_programm) = &self.current_programm {
                    if let Some(call) = &cur_programm.call {
                        if let Err(e) = run_script_in_new_window(call) {
                            println!("Error running programm: \"{}\" Error: {}", call, e);
                            self.status_message =
                                (format!("Execution failed: {}", e), StatusMessageType::Error);
                        }
                    }
                }
                Task::none()
            }
            Message::ManipulateProgramm(manipulation) => {
                if let Some(cur_programm) = &self.current_programm {
                    let script = match &manipulation {
                        ProgrammManipulation::Install => cur_programm.installation.clone(),
                        ProgrammManipulation::Uninstall => cur_programm.deletion.clone(),
                    };
                    return Task::perform(
                        async move { run_script_in_new_window(&script) },
                        move |res| {
                            if let Ok(res) = res {
                                Message::Manipulationresult(manipulation, res)
                            } else {
                                Message::Manipulationresult(manipulation, Err("Fail".to_string()))
                            }
                        },
                    );
                }
                Task::none()
            }
            Message::Manipulationresult(manipulation, result) => {
                match result {
                    Ok(()) => {
                        if let Some(prog) = self.current_programm.as_mut() {
                            prog.status = match manipulation {
                                ProgrammManipulation::Install => ProgrammStatus::Installed,
                                ProgrammManipulation::Uninstall => ProgrammStatus::NotInstalled,
                            }
                        }
                        if let Some(name) = self.current_programm.as_ref().map(|p| p.name.clone()) {
                            if let Some(prog) = self.programms.get_mut(&name) {
                                prog.status = match manipulation {
                                    ProgrammManipulation::Install => ProgrammStatus::Installed,
                                    ProgrammManipulation::Uninstall => ProgrammStatus::NotInstalled,
                                }
                            }
                        }
                        self.status_message = ("Ok!".to_string(), StatusMessageType::Success);
                    }
                    Err(e) => self.status_message = (e, StatusMessageType::Error),
                }
                Task::none()
            }
            Message::ControlMenuBtn(variation) => {
                self.search_selected = false;
                self.cur_menu = match variation {
                    ControlMenuVariations::ExitProgramm => {
                        return iced::window::get_latest().and_then(iced::window::close);
                    }
                    ControlMenuVariations::HelpMenu => {
                        let mut readme_text = String::new();
                        if let Some(mut readme) = File::open("README.md").ok() {
                            readme.read_to_string(&mut readme_text).unwrap_or_else(|_| {
                                readme_text = "Failed to read README.md".to_string();
                                0
                            });
                        } else {
                            readme_text =
                                "Can't find a README.md file, help and docs stored in it."
                                    .to_string();
                        }
                        self.help_md = markdown::parse(&readme_text).collect();
                        ControlMenuVariations::HelpMenu
                    }
                    other => other,
                };
                Task::none()
            }
            Message::AppEvent(given_event) => {
                if let Event::Keyboard(keyboard::Event::KeyPressed {
                    key: _,
                    modified_key,
                    physical_key: _,
                    location: _,
                    modifiers: _,
                    text: _,
                }) = given_event
                {
                    match modified_key {
                        keyboard::Key::Named(named) => match named {
                            keyboard::key::Named::ArrowDown => {
                                self.selected_result_index = min(
                                    self.selected_result_index + 1,
                                    if self.search_programms_names.len() > 0 {
                                        self.search_programms_names.len() - 1
                                    } else {
                                        0
                                    },
                                );
                            }
                            keyboard::key::Named::ArrowUp => {
                                if self.selected_result_index != 0 {
                                    self.selected_result_index -= 1;
                                }
                            }
                            keyboard::key::Named::Enter => {
                                let avaliable_progs = self.programms_startswith(&self.search_text);
                                if let Some(name) = avaliable_progs.get(min(
                                    self.selected_result_index,
                                    if self.search_programms_names.len() > 0 {
                                        self.search_programms_names.len() - 1
                                    } else {
                                        0
                                    },
                                )) {
                                    return self
                                        .update(Message::SelectProgrammFromList(name.clone()));
                                } else {
                                    self.search_selected = false;
                                    self.search_text.clear();
                                    self.selected_result_index = 0;
                                }
                            }
                            keyboard::key::Named::Backspace => {
                                self.search_programms_names =
                                    self.programms_startswith(&self.search_text);
                                self.search_text.pop();
                            }
                            keyboard::key::Named::Escape => {
                                if self.search_selected {
                                    self.search_selected = false;
                                    self.search_text.clear();
                                    self.selected_result_index = 0;
                                } else {
                                    return self.update(Message::ControlMenuBtn(
                                        ControlMenuVariations::ExitProgramm,
                                    ));
                                }
                            }
                            keyboard::key::Named::Space => {
                                if !self.search_selected {
                                    self.search_selected = true;
                                    self.search_text = String::from(" ");
                                    self.search_programms_names =
                                        self.programms_startswith(&self.search_text);
                                } else {
                                    self.search_text += " ";
                                    self.search_programms_names =
                                        self.programms_startswith(&self.search_text);
                                }
                            }
                            _ => {}
                        },
                        keyboard::Key::Character(ch) => {
                            if !self.search_selected {
                                self.search_selected = true;
                                self.search_text = ch.to_string();
                                self.search_programms_names =
                                    self.programms_startswith(&self.search_text);
                            } else {
                                self.search_text += ch.to_string().as_ref();
                                self.search_programms_names =
                                    self.programms_startswith(&self.search_text);
                            }
                        }
                        keyboard::Key::Unidentified => {}
                    }
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let control_menu_list = row![
            button("[ Programms ]")
                .on_press(Message::ControlMenuBtn(
                    ControlMenuVariations::ProgrammsMenu
                ))
                .style(menu_buttons_style),
            button("[ Help ]")
                .on_press(Message::ControlMenuBtn(ControlMenuVariations::HelpMenu))
                .style(menu_buttons_style),
            button("[ Config files ]")
                .on_press(Message::ControlMenuBtn(ControlMenuVariations::ConfigsMenu))
                .style(menu_buttons_style),
            iced::widget::Space::with_width(Length::Fill),
            button("[ Exit ]")
                .on_press(Message::ControlMenuBtn(ControlMenuVariations::ExitProgramm))
                .style(menu_buttons_style),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding(padding::left(20).right(20).bottom(3).top(3))
        .width(Length::Fill);

        let control_menu_container = container(control_menu_list)
            .align_y(Alignment::Center)
            .style(containers_style)
            .width(Length::Fill)
            .height(Length::FillPortion(2));

        let bottom_info_line = row![
            text(format!("Loaded config: {}", self.config_name)).size(14),
            iced::widget::Space::with_width(Length::Fill),
            match &self.status_message {
                (message, StatusMessageType::Error) => text(message).size(14).color(color_error()),
                (message, StatusMessageType::Success) =>
                    text(message).size(14).color(color_success()),
                (message, StatusMessageType::Info) => text(message).size(14).color(color_info()),
            }
        ]
        .height(Length::FillPortion(1))
        .spacing(5)
        .width(Length::Fill);

        let cur_menu = container(match self.cur_menu {
            ControlMenuVariations::HelpMenu => self.help_scene(),
            ControlMenuVariations::ProgrammsMenu => self.main_scene(),
            ControlMenuVariations::ConfigsMenu => self.configs_scene(),
            ControlMenuVariations::ExitProgramm => iced::widget::text!("Unreacheable!").into(),
        })
        .height(Length::FillPortion(37))
        .width(Length::Fill);

        iced::widget::column![control_menu_container, cur_menu, bottom_info_line]
            .padding(padding::all(10).bottom(0))
            .spacing(8)
            .into()
    }

    fn main_scene(&self) -> Element<Message> {
        let programms_scrollable_list = scrollable(iced::widget::column(
            self.programms
                .iter()
                .map(|(name, prog)| {
                    button(prog.name.as_str())
                        .on_press(Message::SelectProgrammFromList(name.clone()))
                        .width(Length::Fill)
                        .style(programms_buttons_style(prog.status))
                        .into()
                })
                .collect::<Vec<Element<_>>>(),
        ));

        let programms_list_container = container(programms_scrollable_list)
            .align_x(Alignment::Center)
            .padding(5)
            .style(containers_style)
            .width(Length::FillPortion(2))
            .height(Length::Fill);

        let programm_actions = row![
            button("Run").on_press(Message::RunProgrammDefault),
            button("Open folder").on_press(Message::OpenContainingFolder),
            button("Docs").on_press(Message::OpenDocsOnline),
            if let Some(prog) = &self.current_programm {
                match prog.status {
                    ProgrammStatus::Installed => button("Uninstall")
                        .on_press(Message::ManipulateProgramm(ProgrammManipulation::Uninstall)),
                    ProgrammStatus::NotInstalled => button("Install")
                        .on_press(Message::ManipulateProgramm(ProgrammManipulation::Install)),
                }
            } else {
                button("Select a program")
            }
        ]
        .padding(padding::left(20))
        .spacing(10);

        let programm_actions_container = container(programm_actions)
            .style(containers_style)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .height(Length::FillPortion(1));

        let description_and_docs_md = markdown::view(
            &self.current_programm_markdown,
            markdown::Settings::default(),
            markdwon_style(),
        )
        .map(Message::DescriptionAndDocsLinkClicked);

        let description_and_docs_container = container(description_and_docs_md)
            .style(containers_style)
            .padding(padding::left(20).right(20))
            .height(Length::FillPortion(14));

        let description_elements =
            iced::widget::column![programm_actions_container, description_and_docs_container,];

        let description_container = container(description_elements)
            .align_x(Alignment::Center)
            .padding(padding::all(10).bottom(0))
            .style(containers_style)
            .width(Length::FillPortion(5))
            .height(Length::Fill);

        let main_view = row![programms_list_container, description_container].spacing(8);

        if self.search_selected {
            stack![main_view, self.search_bar_overlapscene()].into()
        } else {
            stack![main_view].into()
        }
    }

    fn search_bar_overlapscene(&self) -> Element<Message> {
        let search_bar = column![
            container(
                text(if self.search_text.len() > 0 {
                    &self.search_text
                } else {
                    "=>_Programm name_"
                })
                .size(18)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .height(Length::Fill)
            )
            .style(containers_style)
            .width(Length::Fill)
            .height(Length::FillPortion(1)),
            container(scrollable(column(
                self.search_programms_names
                    .iter()
                    .map(|name| button(name.as_str())
                        .on_press(Message::SelectProgrammFromList(name.to_owned()))
                        .width(Length::Fill)
                        .style(programms_buttons_style(ProgrammStatus::Installed))
                        .into())
                    .collect::<Vec<Element<_>>>()
            )))
            .style(containers_style)
            .width(Length::Fill)
            .height(Length::FillPortion(9))
        ];

        container(row![
            iced::widget::Space::with_width(Length::FillPortion(1)),
            search_bar.width(Length::FillPortion(2)).padding(5),
            iced::widget::Space::with_width(Length::FillPortion(1)),
        ])
        .style(|_t| {
            let default_container_style = containers_style(_t);
            container::Style {
                text_color: default_container_style.text_color,
                background: Some(Background::Color(color!(0x34, 0x3D, 0x4B, 0.6))),
                border: border::Border::default(),
                shadow: default_container_style.shadow,
            }
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(5)
        .into()
    }

    fn help_scene(&self) -> Element<Message> {
        markdown::view(
            &self.help_md,
            markdown::Settings::default(),
            markdwon_style(),
        )
        .map(Message::DescriptionAndDocsLinkClicked)
        .into()
    }

    fn configs_scene(&self) -> Element<Message> {
        iced::widget::text!("Here will be configuration files menu")
            .size(30)
            .into()
    }

    fn programms_startswith(&self, startswith: &str) -> Vec<String> {
        let mut res = Vec::new();
        for (name, _) in &self.programms {
            if name.starts_with(startswith) {
                res.push(name.clone());
            }
        }
        res
    }
}

#[derive(Default, Deserialize)]
pub struct ConfigLoad {
    name: String,
    programms: Vec<Programm>,
}

pub fn load_config(
    config_name: &str,
) -> Result<(BTreeMap<String, Programm>, String), Box<dyn Error>> {
    let file = File::open(config_name)?;
    let reader = BufReader::new(file);
    let r: ConfigLoad = serde_json::from_reader(reader)?;
    let mut res = BTreeMap::new();
    for prog in r.programms {
        res.insert(prog.name.clone(), prog);
    }
    Ok((res, r.name))
}

#[derive(Clone, Copy, Deserialize, Default)]
pub enum ProgrammStatus {
    Installed,
    #[default]
    NotInstalled,
}

fn programms_buttons_style(
    status: ProgrammStatus,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_t: &Theme, s: button::Status| -> button::Style {
        button::Style {
            background: match (status, s) {
                (ProgrammStatus::Installed, button::Status::Hovered) => {
                    Some(Background::Color(color!(0xA3, 0xBE, 0x8C)))
                }
                (ProgrammStatus::Installed, _) => Some(Background::Color(color!(0x6B, 0x82, 0x5F))),
                (ProgrammStatus::NotInstalled, button::Status::Hovered) => {
                    Some(Background::Color(color!(0xBF, 0x61, 0x6A)))
                }
                (ProgrammStatus::NotInstalled, _) => {
                    Some(Background::Color(color!(0x8C, 0x46, 0x4F)))
                }
            },
            text_color: color!(0xE5, 0xE9, 0xF0),
            border: Border::default().rounded(4),
            shadow: Shadow::default(),
        }
    }
}

fn menu_buttons_style(_t: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: match s {
            button::Status::Hovered => Some(Background::Color(color!(0x5E, 0x81, 0xAC))),
            _ => Some(Background::Color(color!(0x4C, 0x56, 0x6A))),
        },
        text_color: color!(0xD8, 0xDE, 0xE9),
        border: Border::default().rounded(4),
        shadow: Shadow::default(),
    }
}

fn containers_style(_t: &Theme) -> container::Style {
    let bor = border::Border::default()
        .rounded(8)
        .width(1.5)
        .color(color!(0x4C, 0x56, 0x6A));
    container::Style {
        background: Some(Background::Color(color!(0x34, 0x3D, 0x4B))),
        border: bor,
        text_color: Some(color!(0xD8, 0xDE, 0xE9)),
        shadow: Shadow::default(),
    }
}

fn markdwon_style() -> markdown::Style {
    markdown::Style {
        inline_code_highlight: Highlight {
            background: Background::Color(color!(0x88, 0xC0, 0xD0)),
            border: Border::default(),
        },
        inline_code_padding: Padding::from(4),
        inline_code_color: color!(0x88, 0xC0, 0xD0),
        link_color: color!(0x81, 0xA1, 0xC1),
    }
}

pub fn color_error() -> iced::Color {
    color!(0xBF, 0x61, 0x6A)
}

pub fn color_success() -> iced::Color {
    color!(0xA3, 0xBE, 0x8C)
}

pub fn color_info() -> iced::Color {
    color!(0xD8, 0xDE, 0xE9)
}

fn run_script_in_new_window(script: &str) -> Result<Result<(), String>, String> {
    process::Command::new("pwsh")
        .args([
            "-Command",
            &format!(
                "Start-Process pwsh -ArgumentList \'-Command\', \'{}\'",
                script
            ),
        ])
        .output()
        .map(|output| {
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Installation failed: {:?}",
                    String::from_utf8(output.stderr)
                ))
            }
        })
        .map_err(|e| e.to_string())
}
