use super::wrapper::*;
use std::{
    collections::{HashMap, VecDeque},
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
pub enum BoolParamLayout {
    Held,
    Checkbox,
}

impl BoolParamLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::BUTTON => Self::Held,
            WWidgetDeclType::CHECK_BUTTON => Self::Checkbox,
            _ => panic!("Not a ButtonLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NumParamLayout {
    NumEntry,
    HorizontalSlider,
    VerticalSlider,
}

impl NumParamLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::NUM_ENTRY => Self::NumEntry,
            WWidgetDeclType::HORIZONTAL_SLIDER => Self::HorizontalSlider,
            WWidgetDeclType::VERTICAL_SLIDER => Self::VerticalSlider,
            _ => panic!("Not a NumParamLayout"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NumDisplayLayout {
    Horizontal,
    Vertical,
}

impl NumDisplayLayout {
    fn from_decl_type(typ: WWidgetDeclType) -> Self {
        match typ {
            WWidgetDeclType::HORIZONTAL_BARGRAPH => Self::Horizontal,
            WWidgetDeclType::VERTICAL_BARGRAPH => Self::Vertical,
            _ => panic!("Not a NumDisplayLayout"),
        }
    }
}

#[derive(Debug)]
/// Metadata for floating-point numerical widgets (common to DspWidget::NumParam
/// and NumDisplay)
pub struct NumMetadata {
    /// A suffix to display with the value
    pub unit: Option<String>,
    /// Which scale (lin, log, exp) to use
    pub scale: WidgetScale,
    /// Whether to show the widget at all
    pub hidden: bool,
    /// A text to show when hovering the widget
    pub tooltip: Option<String>,
}

#[derive(Debug)]
/// A Rust representation of some widget in a Faust DSP script. The
/// correspondence is not one-to-one: Faust widgets are grouped in functionally
/// identical variants (same value type, same metadata) which only differ in
/// their visual representation. The visual representation to use is contained
/// for each variant in a 'layout' field whose type depends on the variant (like
/// NumParamLayout for a NumParam, ie. a Faust slider or nentry), and can be
/// completed by an additional 'style' field (read from the widget's metadata in
/// the script).
///
/// The extra metadata listed for some widget in the DSP script is also
/// represented by the inner fields of its corresponding DspWidget variant (how
/// exactly depends on the variant).
///
/// The "zone" (Z type param) corresponds to some reference to use to read &
/// write the current value attached to this widget. Internally, it's a C
/// pointer (to some internal memory region of the DSP object) that can be read
/// or written atomically.
pub enum DspWidget<Z> {
    /// Widgets containing others (tgroup, hgroup and vgroup in Faust)
    ///
    /// The "box" term is the one used in the libfaust API
    Box {
        layout: BoxLayout,
        label: String,
        inner: Vec<DspWidget<Z>>,
    },
    /// Widgets corresponding to interactive boolean parameters (button and
    /// checkbox in Faust)
    BoolParam {
        layout: BoolParamLayout,
        label: String,
        zone: Z,
        hidden: bool,
        tooltip: Option<String>,
    },
    /// Widgets corresponding to interactive numerical floating-point parameters
    /// (hslider, vslider and nentry in Faust), which can take continuous or
    /// discrete values within some range
    NumParam {
        layout: NumParamLayout,
        style: NumParamStyle,
        label: String,
        zone: Z,
        init: f32,
        min: f32,
        max: f32,
        step: f32,
        metadata: NumMetadata,
    },
    /// Widgets corresponding to non-interactive numerical floating-point
    /// displays (hbargraph and vbargraph in Faust), which can show any value
    /// within some range
    NumDisplay {
        layout: NumDisplayLayout,
        style: NumDisplayStyle,
        label: String,
        zone: Z,
        min: f32,
        max: f32,
        metadata: NumMetadata,
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
            DspWidget::BoolParam { label, .. } => label,
            DspWidget::NumParam { label, .. } => label,
            DspWidget::NumDisplay { label, .. } => label,
        }
    }
}

#[derive(Debug)]
/// Which GUI element to use to display a slider or nentry
pub enum NumParamStyle {
    /// Just use the 'layout' field
    FromLayout,
    /// Use a circular knob
    Knob,
    /// Use a dropdown list of labels which each correspond to a specific value
    /// (for discrete-valued parameters)
    Menu(HashMap<String, f32>),
    /// Use a set of labelled, mutally-exclusive radio buttons (for
    /// discrete-valued parameters)
    Radio(HashMap<String, f32>),
}

#[derive(Debug)]
/// Which GUI element to use to display a bargraph
pub enum NumDisplayStyle {
    /// Just use the 'layout' field
    FromLayout,
    /// Encode the value as the color of a led-like display
    Led,
    /// Just show the value as plain text with no extra fuss
    Numerical,
}

#[derive(Debug)]
/// Which scale (linear, logarithmic or exponential) to use to display a slider
/// or bargraph
pub enum WidgetScale {
    Lin,
    Log,
    Exp,
}

enum WidgetStyle {
    // For sliders/nentries:
    Param(NumParamStyle),
    // For bargraphs:
    Disp(NumDisplayStyle),
}

enum MetadataElem {
    Style(WidgetStyle),
    Scale(WidgetScale),
    Hidden(bool),
    Unit(String),
    Tooltip(String),
}

pub struct DspWidgetsBuilder {
    widget_decls: VecDeque<(String, WWidgetDecl)>,
    metadata_map: HashMap<*mut f32, Vec<MetadataElem>>,
}

/// A memory zone corresponding to some parameter's current value
pub trait Zone {
    unsafe fn from_zone_ptr(ptr: *mut f32) -> Self;

    fn cur_value(&self) -> f32;
}

impl<'a> Zone for &'a mut f32 {
    unsafe fn from_zone_ptr(ptr: *mut f32) -> Self {
        ptr.as_mut().unwrap()
    }

    fn cur_value(&self) -> f32 {
        **self
    }
}

impl DspWidgetsBuilder {
    pub fn new() -> Self {
        Self {
            widget_decls: VecDeque::new(),
            metadata_map: HashMap::new(),
        }
    }

    pub fn build_widgets<Z: Zone>(mut self, widget_list: &mut Vec<DspWidget<Z>>) {
        self.build_widgets_rec(widget_list);
        assert!(
            self.widget_decls.is_empty(),
            "Some widget declarations haven't been consumed"
        );
    }

    /// To be called _after_ faust's buildUserInterface has finished, ie. after
    /// w_createUIs has finished. 'a is the lifetime of the DSP itself
    fn build_widgets_rec<Z: Zone>(&mut self, cur_level: &mut Vec<DspWidget<Z>>) {
        use MetadataElem as ME;
        use WWidgetDeclType as W;
        let mut empty_vec = Vec::new();
        while let Some((label, decl)) = self.widget_decls.pop_front() {
            // Getting metadata
            let md_elems = self
                .metadata_map
                .get_mut(&decl.zone)
                .unwrap_or(&mut empty_vec);
            let mut style = None;
            let mut metadata = NumMetadata {
                unit: None,
                scale: WidgetScale::Lin,
                hidden: false,
                tooltip: None,
            };
            while let Some(elem) = md_elems.pop() {
                match elem {
                    ME::Style(s) => style = Some(s),
                    ME::Scale(s) => metadata.scale = s,
                    ME::Hidden(h) => metadata.hidden = h,
                    ME::Unit(u) => metadata.unit = Some(u),
                    ME::Tooltip(t) => metadata.tooltip = Some(t),
                }
            }

            let mut widget = match decl.typ {
                W::CLOSE_BOX => return,
                W::TAB_BOX | W::HORIZONTAL_BOX | W::VERTICAL_BOX => DspWidget::Box {
                    layout: BoxLayout::from_decl_type(decl.typ),
                    label,
                    inner: vec![],
                },
                W::BUTTON | W::CHECK_BUTTON => DspWidget::BoolParam {
                    layout: BoolParamLayout::from_decl_type(decl.typ),
                    label,
                    zone: unsafe { Zone::from_zone_ptr(decl.zone) },
                    hidden: metadata.hidden,
                    tooltip: metadata.tooltip,
                },
                W::HORIZONTAL_SLIDER | W::VERTICAL_SLIDER | W::NUM_ENTRY => DspWidget::NumParam {
                    layout: NumParamLayout::from_decl_type(decl.typ),
                    style: match style {
                        Some(WidgetStyle::Param(s)) => s,
                        _ => NumParamStyle::FromLayout,
                    },
                    label,
                    zone: unsafe { Zone::from_zone_ptr(decl.zone) },
                    init: decl.init,
                    min: decl.min,
                    max: decl.max,
                    step: decl.step,
                    metadata,
                },
                W::HORIZONTAL_BARGRAPH | W::VERTICAL_BARGRAPH => DspWidget::NumDisplay {
                    layout: NumDisplayLayout::from_decl_type(decl.typ),
                    style: match style {
                        Some(WidgetStyle::Disp(s)) => s,
                        _ => NumDisplayStyle::FromLayout,
                    },
                    label,
                    zone: unsafe { Zone::from_zone_ptr(decl.zone) },
                    min: decl.min,
                    max: decl.max,
                    metadata,
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

// The cpp wrapper-lib will link with these:

#[no_mangle]
extern "C" fn rs_declare_widget(
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

#[no_mangle]
extern "C" fn rs_declare_metadata(
    builder_ptr: *mut c_void,
    zone_ptr: *mut f32,
    key_ptr: *const c_char,
    value_ptr: *const c_char,
) {
    use MetadataElem as ME;
    use WidgetStyle as WS;
    let builder = unsafe { (builder_ptr as *mut DspWidgetsBuilder).as_mut() }.unwrap();
    let key = unsafe { CStr::from_ptr(key_ptr) }.to_str().unwrap().trim();
    let value = unsafe { CStr::from_ptr(value_ptr) }
        .to_str()
        .unwrap()
        .trim();
    let opt_elem = match key {
        "unit" => Some(ME::Unit(value.to_owned())),
        "tooltip" => Some(ME::Tooltip(value.to_owned())),
        "style" => match value {
            "knob" => Some(ME::Style(WS::Param(NumParamStyle::Knob))),
            "led" => Some(ME::Style(WS::Disp(NumDisplayStyle::Led))),
            "numerical" => Some(ME::Style(WS::Disp(NumDisplayStyle::Numerical))),
            _ => {
                if value.starts_with("menu") {
                    let dict = parse_metadata_dict(&value[4..]);
                    Some(ME::Style(WS::Param(NumParamStyle::Menu(dict))))
                } else if value.starts_with("radio") {
                    let dict = parse_metadata_dict(&value[5..]);
                    Some(ME::Style(WS::Param(NumParamStyle::Radio(dict))))
                } else {
                    None
                }
            }
        },
        "scale" => match value {
            "lin" => Some(ME::Scale(WidgetScale::Lin)),
            "log" => Some(ME::Scale(WidgetScale::Log)),
            "exp" => Some(ME::Scale(WidgetScale::Exp)),
            _ => None,
        },
        "hidden" => match value {
            "0" => Some(ME::Hidden(false)),
            "1" => Some(ME::Hidden(true)),
            _ => None,
        },
        _ => None,
    };
    if let Some(elem) = opt_elem {
        let map = &mut builder.metadata_map;
        if !map.contains_key(&zone_ptr) {
            map.insert(zone_ptr, Vec::new());
        }
        map.get_mut(&zone_ptr).unwrap().push(elem);
    }
}

fn parse_metadata_dict(s: &str) -> HashMap<String, f32> {
    let trimmed = s.trim();
    let without_brackets = &trimmed[1..trimmed.len() - 1].trim();
    let mut map = HashMap::new();
    for kv_pair in without_brackets.split(";") {
        let mut it = kv_pair.split(":");
        let mut k = it.next().unwrap().trim();
        let v = it.next().unwrap().trim();
        k = &k[1..k.len() - 1]; // Remove the quotes
        map.insert(k.to_owned(), v.parse().unwrap());
    }
    map
}
