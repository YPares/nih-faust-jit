use super::wrapper::*;
use std::{
    collections::{BTreeMap, VecDeque},
    ffi::{c_char, c_void, CStr},
    hash::{Hash, Hasher},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BoxLayout {
    Tab { selected: usize },
    Horizontal,
    Vertical,
}

impl BoxLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::HORIZONTAL_BOX => Self::Horizontal,
            WWidgetDeclType::VERTICAL_BOX => Self::Vertical,
            WWidgetDeclType::TAB_BOX => Self::Tab { selected: 0 },
            _ => panic!("Not a BoxLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ButtonLayout {
    Held,
    Checkbox,
}

impl ButtonLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::BUTTON => Self::Held,
            WWidgetDeclType::CHECK_BUTTON => Self::Checkbox,
            _ => panic!("Not a ButtonLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NumericLayout {
    NumEntry,
    HorizontalSlider,
    VerticalSlider,
}

impl NumericLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::NUM_ENTRY => Self::NumEntry,
            WWidgetDeclType::HORIZONTAL_SLIDER => Self::HorizontalSlider,
            WWidgetDeclType::VERTICAL_SLIDER => Self::VerticalSlider,
            _ => panic!("Not a NumericLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BargraphLayout {
    Horizontal,
    Vertical,
}

impl BargraphLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::HORIZONTAL_BARGRAPH => Self::Horizontal,
            WWidgetDeclType::VERTICAL_BARGRAPH => Self::Vertical,
            _ => panic!("Not a BargraphLayout"),
        }
    }
}

#[derive(Debug)]
pub enum DspWidget<Z> {
    Box {
        layout: BoxLayout,
        label: String,
        inner: Vec<DspWidget<Z>>,
    },
    Button {
        layout: ButtonLayout,
        label: String,
        zone: Z,
    },
    Numeric {
        layout: NumericLayout,
        label: String,
        zone: Z,
        init: f32,
        min: f32,
        max: f32,
        step: f32,
    },
    Bargraph {
        layout: BargraphLayout,
        label: String,
        zone: Z,
        min: f32,
        max: f32,
    },
    // Soundfile {
    //     label: String,
    //     path: PathBuf,
    //     // TODO
    // },
}

impl<Z> DspWidget<Z> {
    pub fn label(&self) -> &str {
        match self {
            DspWidget::Box { label, .. } => label,
            DspWidget::Button { label, .. } => label,
            DspWidget::Numeric { label, .. } => label,
            DspWidget::Bargraph { label, .. } => label,
        }
    }
}

/// A memory zone corresponding to some parameter's current value. The name
/// "zone" comes from Faust
pub trait ParamZone {
    unsafe fn from_ptr(ptr: *mut f32) -> Self;
    fn get(&self) -> f32;
    fn set(&mut self, x: f32);
}

impl<'a> ParamZone for &'a mut f32 {
    unsafe fn from_ptr(ptr: *mut f32) -> Self {
        ptr.as_mut().unwrap()
    }

    fn get(&self) -> f32 {
        **self
    }

    fn set(&mut self, x: f32) {
        **self = x;
    }
}

fn write_widgets_zones_rec<Z: ParamZone>(
    path_vec: &mut Vec<String>,
    widgets: &[DspWidget<Z>],
    map: &mut BTreeMap<String, String>,
) {
    for w in widgets {
        path_vec.push(w.label().to_owned());
        match w {
            DspWidget::Box { inner, .. } => {
                write_widgets_zones_rec(path_vec, inner, map);
            }
            DspWidget::Button { zone, .. }
            | DspWidget::Numeric { zone, .. }
            | DspWidget::Bargraph { zone, .. } => {
                let cur_path = path_vec.join("/");
                map.insert(cur_path, zone.get().to_string());
            }
        }
        path_vec.pop();
    }
}

/// Serialize the current values of the widgets' zones into a map
pub fn write_widgets_zones<Z: ParamZone>(
    widgets: &[DspWidget<Z>],
    map: &mut BTreeMap<String, String>,
) {
    write_widgets_zones_rec(&mut Vec::new(), widgets, map);
}

fn load_widgets_zones_rec<Z: ParamZone>(
    path_vec: &mut Vec<String>,
    widgets: &mut [DspWidget<Z>],
    map: &BTreeMap<String, String>,
    missing: &mut Vec<String>,
    failed_to_parse: &mut Vec<String>,
) {
    for w in widgets {
        path_vec.push(w.label().to_owned());
        match w {
            DspWidget::Box { inner, .. } => {
                load_widgets_zones_rec(path_vec, inner, map, missing, failed_to_parse);
            }
            DspWidget::Button { zone, .. }
            | DspWidget::Numeric { zone, .. }
            | DspWidget::Bargraph { zone, .. } => {
                let cur_path = path_vec.join("/");
                match map.get(&cur_path) {
                    None => missing.push(cur_path),
                    Some(val_str) => match val_str.parse() {
                        Ok(val) => zone.set(val),
                        Err(_) => failed_to_parse.push(cur_path),
                    },
                }
            }
        }
        path_vec.pop();
    }
}

/// Override the current values of the widgets' zones from a map
pub fn load_widgets_zone<Z: ParamZone>(
    widgets: &mut [DspWidget<Z>],
    map: &BTreeMap<String, String>,
) -> Result<(), String> {
    let mut missing = Vec::new();
    let mut failed_to_parse = Vec::new();
    load_widgets_zones_rec(
        &mut Vec::new(),
        widgets,
        map,
        &mut missing,
        &mut failed_to_parse,
    );
    if missing.is_empty() && failed_to_parse.is_empty() {
        Ok(())
    } else {
        Err(
            format!("Stored state is invalid:\n  - Missing paths: {:?}\n  - Paths that failed to parse: {:?}", missing, failed_to_parse)
        )
    }
}

pub struct DspWidgetsBuilder {
    widget_decls: VecDeque<(String, WWidgetDecl)>,
}

impl DspWidgetsBuilder {
    pub fn new() -> Self {
        Self {
            widget_decls: VecDeque::new(),
        }
    }

    pub fn build_widgets<Z: ParamZone>(mut self, widget_list: &mut Vec<DspWidget<Z>>) {
        self.build_widgets_rec(widget_list);
        assert!(
            self.widget_decls.is_empty(),
            "Some widget declarations haven't been consumed"
        );
    }

    /// To be called _after_ faust's buildUserInterface has finished, ie. after
    /// w_createUIs has finished. 'a is the lifetime of the DSP itself
    fn build_widgets_rec<Z: ParamZone>(&mut self, cur_level: &mut Vec<DspWidget<Z>>) {
        use WWidgetDeclType as W;
        while let Some((label, decl)) = self.widget_decls.pop_front() {
            let mut widget = match decl.typ {
                W::CLOSE_BOX => return,
                W::TAB_BOX | W::HORIZONTAL_BOX | W::VERTICAL_BOX => DspWidget::Box {
                    layout: BoxLayout::from_decl_type(decl.typ),
                    label,
                    inner: vec![],
                },
                W::BUTTON | W::CHECK_BUTTON => DspWidget::Button {
                    layout: ButtonLayout::from_decl_type(decl.typ),
                    label,
                    zone: unsafe { ParamZone::from_ptr(decl.zone) },
                },
                W::HORIZONTAL_SLIDER | W::VERTICAL_SLIDER | W::NUM_ENTRY => DspWidget::Numeric {
                    layout: NumericLayout::from_decl_type(decl.typ),
                    label,
                    zone: unsafe { ParamZone::from_ptr(decl.zone) },
                    init: decl.init,
                    min: decl.min,
                    max: decl.max,
                    step: decl.step,
                },
                W::HORIZONTAL_BARGRAPH | W::VERTICAL_BARGRAPH => DspWidget::Bargraph {
                    layout: BargraphLayout::from_decl_type(decl.typ),
                    label,
                    zone: unsafe { ParamZone::from_ptr(decl.zone) },
                    min: decl.min,
                    max: decl.max,
                },
            };
            if let DspWidget::Box { inner, .. } = &mut widget {
                // We recurse, so as to add to the newly opened box:
                self.build_widgets_rec(inner);
            }
            cur_level.push(widget);
        }
    }
}

pub(crate) extern "C" fn widget_decl_callback(
    builder_ptr: *mut c_void,
    label_ptr: *const c_char,
    decl: WWidgetDecl,
) {
    let builder = unsafe { (builder_ptr as *mut DspWidgetsBuilder).as_mut() }.unwrap();
    let c_label = unsafe { CStr::from_ptr(label_ptr) };
    let label = match c_label.to_str() {
        Ok("0x00") => "".to_string(),
        Ok(s) => s.to_string(),
        _ => {
            // Label couldn't parse to utf8. We just hash the raw CStr to get
            // some label:
            let mut state = std::hash::DefaultHasher::new();
            c_label.hash(&mut state);
            state.finish().to_string()
        }
    };
    builder.widget_decls.push_back((label, decl));
}
