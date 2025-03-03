use std::{collections::BTreeMap, error::Error, fs::File, io::BufReader, path::Path, process};
use serde::Deserialize;
use iced::{border, color, padding, theme::Palette, widget::{button, container, markdown::{self, Highlight}, row, scrollable, text}, window::{settings::PlatformSpecific, Position}, Alignment, Background, Border, Element, Length, Padding, Shadow, Size, Task, Theme};

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
    iced::application("Win tool box", WinToolBox::update, WinToolBox::view).window(iced::window::Settings{
        size: Size{ width: 1200., height: 800. },
        position: Position::Centered,
        min_size: None,
        max_size: None,
        visible: true,
        resizable: true,
        decorations: false,
        transparent: true,
        level: iced::window::Level::Normal,
        icon: {
            let img = image::open("wintoolbox.jpg").expect("Failed to open icon");
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            Some(iced::window::icon::from_rgba(rgba.into_raw(), width, height).expect("Failed to create icon"))
        },
        platform_specific: PlatformSpecific::default(),
        exit_on_close_request: true,
    })
        .theme(|_| custom_theme())
        .run_with(WinToolBox::new)
}

#[derive(Default)]
struct WinToolBox {
    current_programm_markdown: Vec<markdown::Item>,
    current_programm: Option<Programm>,
    programms: BTreeMap<String, Programm>,
    config_name: String,
    error_message: Option<String>,
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

#[derive(Debug, Clone)]
enum Message {
    SelectProgrammFromList(String),
    RunProgrammDefault,
    DescriptionAndDocsLinkClicked(markdown::Url),
    OpenContainingFolder,
    OpenDocsOnline,
    HelpMenu,
    SettingsMenu,
    ConfigurationFilesMenu,
    ExitProgramm,
    InstallProgramm,
    UninstallProgramm,
    InstallCompleted(Result<(), String>),
    UninstallCompleted(Result<(), String>),
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
                error_message: None,
            },
            Task::none()
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DescriptionAndDocsLinkClicked(link) => {
                if let Err(e) = opener::open(link.as_str()) {
                    self.error_message = Some(format!("Can't open link: {}", e));
                }
                Task::none()
            }
            Message::OpenDocsOnline => {
                if let Some(cur_programm) = &self.current_programm {
                    if let Some(docs_link) = &cur_programm.docs_link {
                        if let Err(e) = opener::open(docs_link) {
                            self.error_message = Some(format!("Can't open link: {}", e));
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
                                let path_str = String::from_utf8(output.stdout).unwrap_or_default().trim().to_string();
                                let first_path = path_str.lines().next().unwrap_or("");
                                let folder_path = Path::new(first_path).parent().unwrap_or(Path::new(""));
                                if let Err(e) = process::Command::new("explorer").arg(folder_path).spawn() {
                                    self.error_message = Some(format!("Can't open explorer: {}", e));
                                }
                            }
                            _ => self.error_message = Some(format!("Program not found: {:#?}", output)),
                        }
                    }
                }
                Task::none()
            }
            Message::SelectProgrammFromList(select_prog_name) => {
                for (prog_name, prog) in &self.programms {
                    if *prog_name == select_prog_name {
                        self.current_programm_markdown = markdown::parse(&prog.description_md).collect();
                        self.current_programm = Some(prog.clone());
                    }
                }
                Task::none()
            }
            Message::RunProgrammDefault => {
                if let Some(cur_programm) = &self.current_programm {
                    if let Some(call) = &cur_programm.call {
                        if let Err(e) = process::Command::new("pwsh").args(&["-Command", "Start-Process", call]).spawn() {
                            println!("Error running programm: \"{}\" Error: {}", call, e);
                            self.error_message = Some(format!("Execution failed: {}", e));
                        }
                    }
                }
                Task::none()
            }
            Message::InstallProgramm => {
                if let Some(cur_programm) = &self.current_programm {
                    let install_cmd = cur_programm.installation.clone();
                    return Task::perform(
                        async move {
                            process::Command::new("pwsh")
                                .args(&["-Command", "Start-Process"])
                                .arg(install_cmd)
                                .output()
                                .map(|output| if output.status.success() { Ok(()) } else { Err(format!("Installation failed: {:?}", String::from_utf8(output.stderr))) })
                                .map_err(|e| e.to_string())
                        },
                        |res| {
                            if let Ok(res) = res {
                                Message::InstallCompleted(res)
                            } else {
                                Message::InstallCompleted(Err("Fail".to_string()))
                            }
                        },
                    );
                }
                Task::none()
            }
            Message::UninstallProgramm => {
                if let Some(cur_programm) = &self.current_programm {
                    let delete_cmd = cur_programm.deletion.clone();
                    return Task::perform(
                        async move {
                            process::Command::new("pwsh")
                                .args(&["-Command", "Start-Process"])
                                .args(delete_cmd.split(' '))
                                .output()
                                .map(|output| if output.status.success() { Ok(()) } else { Err(format!("Uninstallation failed: {:?}", String::from_utf8(output.stderr))) })
                                .map_err(|e| e.to_string())
                        },
                        |res| {
                            if let Ok(res) = res {
                                Message::UninstallCompleted(res)
                            } else {
                                Message::UninstallCompleted(Err("Fail".to_string()))
                            }
                        },
                    );
                }
                Task::none()
            }
            Message::InstallCompleted(result) => {
                match result {
                    Ok(()) => {
                        if let Some(prog) = self.current_programm.as_mut() {
                            prog.status = ProgrammStatus::Installed;
                        }
                        if let Some(name) = self.current_programm.as_ref().map(|p| p.name.clone()) {
                            if let Some(prog) = self.programms.get_mut(&name) {
                                prog.status = ProgrammStatus::Installed;
                            }
                        }
                        self.error_message = None;
                    }
                    Err(e) => self.error_message = Some(e),
                }
                Task::none()
            }
            Message::UninstallCompleted(result) => {
                match result {
                    Ok(()) => {
                        if let Some(prog) = self.current_programm.as_mut() {
                            prog.status = ProgrammStatus::NotInstalled;
                        }
                        if let Some(name) = self.current_programm.as_ref().map(|p| p.name.clone()) {
                            if let Some(prog) = self.programms.get_mut(&name) {
                                prog.status = ProgrammStatus::NotInstalled;
                            }
                        }
                        self.error_message = None;
                    }
                    Err(e) => self.error_message = Some(e),
                }
                Task::none()
            }
            Message::HelpMenu => {
                println!("Not implemented");
                Task::none()
            }
            Message::SettingsMenu => {
                println!("Not implemented");
                Task::none()
            }
            Message::ConfigurationFilesMenu => {
                println!("Not implemented");
                Task::none()
            }
            Message::ExitProgramm => {
                process::exit(0)
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let containers_style = |_t: &Theme| -> container::Style {
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
        };

        let menu_buttons_style = |_t: &Theme, s: button::Status| -> button::Style {
            button::Style {
                background: match s {
                    button::Status::Hovered => Some(Background::Color(color!(0x5E, 0x81, 0xAC))),
                    _ => Some(Background::Color(color!(0x4C, 0x56, 0x6A))),
                },
                text_color: color!(0xD8, 0xDE, 0xE9),
                border: Border::default().rounded(4),
                shadow: Shadow::default(),
            }
        };

        let programms_scrollable_list = scrollable(
            iced::widget::column(
                self.programms.iter().map(|(name, prog)| {
                    button(prog.name.as_str())
                        .on_press(Message::SelectProgrammFromList(name.clone()))
                        .width(Length::Fill)
                        .style(programms_buttons_style(prog.status)).into()
                }).collect::<Vec<Element<_>>>()
            )
        );

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
                    ProgrammStatus::Installed => button("Uninstall").on_press(Message::UninstallProgramm),
                    ProgrammStatus::NotInstalled => button("Install").on_press(Message::InstallProgramm),
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
            markdown::Style {
                inline_code_highlight: Highlight {
                    background: Background::Color(color!(0x88, 0xC0, 0xD0)),
                    border: Border::default(),
                },
                inline_code_padding: Padding::from(4),
                inline_code_color: color!(0x88, 0xC0, 0xD0),
                link_color: color!(0x81, 0xA1, 0xC1),
            },
        ).map(Message::DescriptionAndDocsLinkClicked);

        let description_and_docs_container = container(description_and_docs_md)
            .style(containers_style)
            .padding(padding::left(20).right(20))
            .height(Length::FillPortion(14));

        let description_elements = iced::widget::column![
            programm_actions_container,
            description_and_docs_container,
        ];

        let description_container = container(description_elements)
            .align_x(Alignment::Center)
            .padding(padding::all(10).bottom(0))
            .style(containers_style)
            .width(Length::FillPortion(5))
            .height(Length::Fill);

        let info_bar_list = row![
            button("[ Settings ]").on_press(Message::SettingsMenu).style(menu_buttons_style),
            button("[ Help ]").on_press(Message::HelpMenu).style(menu_buttons_style),
            button("[ Config files ]").on_press(Message::ConfigurationFilesMenu).style(menu_buttons_style),
            iced::widget::Space::with_width(Length::Fill),
            button("[ Exit ]").on_press(Message::ExitProgramm).style(menu_buttons_style),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding(padding::left(20).right(20).bottom(3).top(3))
        .width(Length::Fill);

        let info_bar_container = container(info_bar_list)
            .align_y(Alignment::Center)
            .style(containers_style)
            .width(Length::Fill)
            .height(Length::FillPortion(2));

        let workspace = row![programms_list_container, description_container]
            .height(Length::FillPortion(37))
            .width(Length::Fill)
            .spacing(8);

        let info_stroke = row![
            text(format!("Loaded config: {}", self.config_name)).size(14),
            iced::widget::Space::with_width(Length::Fill),
            if let Some(error) = &self.error_message {
                text(error).size(14).color(color!(0xBF, 0x61, 0x6A))
            } else {
                text("Ok!").size(14).color(color!(0xA3, 0xBE, 0x8C))
            }
        ]
        .height(Length::FillPortion(1))
        .spacing(5)
        .width(Length::Fill);

        iced::widget::column![
            info_bar_container,
            workspace,
            info_stroke
        ]
        .padding(padding::all(10).bottom(0))
        .spacing(8)
        .into()
    }
}

#[derive(Default, Deserialize)]
pub struct ConfigLoad {
    name: String,
    programms: Vec<Programm>,
}

pub fn load_config(config_name: &str) -> Result<(BTreeMap<String, Programm>, String), Box<dyn Error>> {
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

fn programms_buttons_style(status: ProgrammStatus) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_t: &Theme, s: button::Status| -> button::Style {
        button::Style {
            background: match (status, s) {
                (ProgrammStatus::Installed, button::Status::Hovered) => Some(Background::Color(color!(0xA3, 0xBE, 0x8C))),
                (ProgrammStatus::Installed, _) => Some(Background::Color(color!(0x6B, 0x82, 0x5F))),
                (ProgrammStatus::NotInstalled, button::Status::Hovered) => Some(Background::Color(color!(0xBF, 0x61, 0x6A))),
                (ProgrammStatus::NotInstalled, _) => Some(Background::Color(color!(0x8C, 0x46, 0x4F))),
            },
            text_color: color!(0xE5, 0xE9, 0xF0),
            border: Border::default().rounded(4),
            shadow: Shadow::default(),
        }
    }
}