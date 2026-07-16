use std::cmp::{max, min};
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;
use zellij_tile::prelude::*;

// ========== COLOR SYSTEM ==========

/// Color specification supporting default, 256-color, and RGB
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum ColorSpec {
    /// Use terminal default color
    #[default]
    Default,
    /// 256-color palette index (0-255)
    EightBit(u8),
    /// True color RGB
    Rgb(u8, u8, u8),
}

impl ColorSpec {
    /// Generate ANSI escape code for foreground color
    fn to_ansi_fg(self) -> String {
        match self {
            ColorSpec::Default => String::new(),
            ColorSpec::EightBit(n) => format!("\x1b[38;5;{}m", n),
            ColorSpec::Rgb(r, g, b) => format!("\x1b[38;2;{};{};{}m", r, g, b),
        }
    }

    /// Generate ANSI escape code for background color
    fn to_ansi_bg(self) -> String {
        match self {
            ColorSpec::Default => String::new(),
            ColorSpec::EightBit(n) => format!("\x1b[48;5;{}m", n),
            ColorSpec::Rgb(r, g, b) => format!("\x1b[48;2;{};{};{}m", r, g, b),
        }
    }

    fn is_default(self) -> bool {
        matches!(self, ColorSpec::Default)
    }
}

/// Parse a color value from string
/// Supports:
/// - Named colors: "accent", "dim", "red", etc.
/// - 256-color: "238"
/// - Hex RGB: "#444444" or "#444"
/// - RGB function: "rgb(68,68,68)"
fn parse_color_spec(name: &str) -> ColorSpec {
    let name = name.trim();

    // Check for RGB hex: #RGB or #RRGGBB
    if let Some(hex) = name.strip_prefix('#')
        && let Some((r, g, b)) = parse_hex_color(hex)
    {
        return ColorSpec::Rgb(r, g, b);
    }

    // Check for rgb(r,g,b) syntax
    if let Some(inner) = name.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')'))
        && let Some((r, g, b)) = parse_rgb_func(inner)
    {
        return ColorSpec::Rgb(r, g, b);
    }

    // Check for numeric 256-color
    if let Ok(n) = name.parse::<u8>() {
        return ColorSpec::EightBit(n);
    }

    // Named colors mapped to 256-color approximations
    match name.to_lowercase().as_str() {
        // Default/reset
        "none" | "default" | "reset" => ColorSpec::Default,

        // Theme-like semantic colors (mapped to reasonable 256-color values)
        "accent" | "primary" => ColorSpec::EightBit(39), // Bright blue
        "secondary" => ColorSpec::EightBit(75),          // Light blue
        "tertiary" => ColorSpec::EightBit(141),          // Purple
        "muted" | "quaternary" => ColorSpec::EightBit(245), // Light gray
        "dim" | "dimmed" => ColorSpec::EightBit(240),    // Dark gray

        // Standard colors
        "black" => ColorSpec::EightBit(0),
        "red" | "error" | "warning" => ColorSpec::EightBit(196),
        "green" | "success" | "ok" => ColorSpec::EightBit(82),
        "yellow" => ColorSpec::EightBit(226),
        "blue" => ColorSpec::EightBit(33),
        "magenta" => ColorSpec::EightBit(201),
        "cyan" => ColorSpec::EightBit(51),
        "white" => ColorSpec::EightBit(15),
        "orange" => ColorSpec::EightBit(208),
        "gray" | "grey" => ColorSpec::EightBit(244),
        "pink" => ColorSpec::EightBit(213),
        "purple" => ColorSpec::EightBit(135),

        // Unknown - use default
        _ => ColorSpec::Default,
    }
}

/// Parse hex color: "444444" or "444" -> (r, g, b)
fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    match hex.len() {
        3 => {
            // #RGB -> expand to #RRGGBB
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some((r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

/// Parse "r,g,b" -> (r, g, b)
fn parse_rgb_func(inner: &str) -> Option<(u8, u8, u8)> {
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 3 {
        return None;
    }
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    Some((r, g, b))
}

/// Convert a Zellij theme PaletteColor into our ColorSpec.
fn palette_to_spec(c: PaletteColor) -> ColorSpec {
    match c {
        PaletteColor::Rgb((r, g, b)) => ColorSpec::Rgb(r, g, b),
        PaletteColor::EightBit(n) => ColorSpec::EightBit(n),
    }
}

// ========== AGENT STATE ==========

/// The state an agent (or command) reports for a pane, via the `agent_state` pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AgentState {
    #[default]
    Idle,
    Working,
    Waiting,
    Done,
    Error,
}

impl AgentState {
    fn parse(s: &str) -> AgentState {
        match s.trim().to_lowercase().as_str() {
            "working" => AgentState::Working,
            "waiting" => AgentState::Waiting,
            "done" => AgentState::Done,
            "error" => AgentState::Error,
            _ => AgentState::Idle,
        }
    }

    fn key(self) -> &'static str {
        match self {
            AgentState::Working => "working",
            AgentState::Waiting => "waiting",
            AgentState::Done => "done",
            AgentState::Error => "error",
            AgentState::Idle => "idle",
        }
    }
}

/// How a state is animated in the tab border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anim {
    None,
    Solid,
    Flash,
    Scroll,
}

impl Anim {
    fn parse(s: &str) -> Anim {
        match s.trim().to_lowercase().as_str() {
            "flash" => Anim::Flash,
            "scroll" => Anim::Scroll,
            "solid" => Anim::Solid,
            _ => Anim::None,
        }
    }

    fn is_animated(self) -> bool {
        matches!(self, Anim::Flash | Anim::Scroll)
    }
}

/// Which visual channels a colour (state or focus) is drawn on. A set, so callers
/// can mix e.g. "border glyph". Tokens: fill, border, glyph (alias dot), both, all, none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Channels {
    fill: bool,
    border: bool,
    glyph: bool,
}

impl Channels {
    fn parse(s: &str) -> Channels {
        let mut c = Channels::default();
        for tok in s.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == '+') {
            match tok.trim().to_lowercase().as_str() {
                "fill" => c.fill = true,
                "border" => c.border = true,
                "glyph" | "dot" => c.glyph = true,
                "both" => {
                    c.fill = true;
                    c.border = true;
                }
                "all" => {
                    c.fill = true;
                    c.border = true;
                    c.glyph = true;
                }
                _ => {}
            }
        }
        c
    }
}

/// The latest state reported for a single pane.
#[derive(Debug, Clone, Default)]
struct PaneAgent {
    state: AgentState,
    #[allow(dead_code)]
    agent: String,
    label: String,
}

// ========== STYLE SYSTEM ==========

/// Inline style from #[...] directive
#[derive(Debug, Clone, Default)]
struct InlineStyle {
    fg: ColorSpec,
    bg: ColorSpec,
    bold: bool,
    dim: bool,
    fill: bool,
}

impl InlineStyle {
    /// Generate ANSI escape codes for this style (without reverse - that's handled at line level)
    fn to_ansi(&self) -> String {
        let mut result = String::new();

        // Attributes
        if self.bold {
            result.push_str("\x1b[1m");
        }
        if self.dim {
            result.push_str("\x1b[2m");
        }

        // Colors
        result.push_str(&self.fg.to_ansi_fg());
        result.push_str(&self.bg.to_ansi_bg());

        result
    }

    fn has_any_style(&self) -> bool {
        !self.fg.is_default() || !self.bg.is_default() || self.bold || self.dim || self.fill
    }
}

/// A segment of text with styling
#[derive(Debug, Clone)]
struct StyledSegment {
    text: String,
    style: InlineStyle,
}

impl StyledSegment {
    fn display_width(&self) -> usize {
        self.text.width()
    }
}

/// Collection of styled segments forming a complete styled string
#[derive(Debug, Clone, Default)]
struct StyledText {
    segments: Vec<StyledSegment>,
}

impl StyledText {
    fn new() -> Self {
        Self { segments: vec![] }
    }

    fn push(&mut self, text: String, style: InlineStyle) {
        if !text.is_empty() {
            self.segments.push(StyledSegment { text, style });
        }
    }

    fn display_width(&self) -> usize {
        self.segments.iter().map(|s| s.display_width()).sum()
    }

    /// Render to ANSI-coded string
    fn to_ansi(&self) -> String {
        let mut result = String::new();

        for segment in &self.segments {
            if segment.style.has_any_style() {
                result.push_str("\x1b[0m"); // Reset before applying new style
                result.push_str(&segment.style.to_ansi());
            }
            result.push_str(&segment.text);
        }

        // Reset at end
        if self.segments.iter().any(|s| s.style.has_any_style()) {
            result.push_str("\x1b[0m");
        }

        result
    }

    /// Truncate to fit within max_width display columns
    fn truncate(&self, max_width: usize) -> StyledText {
        if self.display_width() <= max_width {
            return self.clone();
        }

        let mut result = StyledText::new();
        let mut remaining = max_width;

        for segment in &self.segments {
            if remaining == 0 {
                break;
            }

            let seg_width = segment.display_width();
            if seg_width <= remaining {
                result.push(segment.text.clone(), segment.style.clone());
                remaining -= seg_width;
            } else {
                // Truncate this segment
                let mut truncated = String::new();
                let mut width = 0;
                for ch in segment.text.chars() {
                    let ch_width = ch.to_string().width();
                    if width + ch_width > remaining {
                        break;
                    }
                    truncated.push(ch);
                    width += ch_width;
                }
                result.push(truncated, segment.style.clone());
                break;
            }
        }

        result
    }
}

// ========== FORMAT PARSING ==========

/// Token from parsing a tmux-style format string
#[derive(Debug, Clone)]
enum FormatToken {
    /// Style directive: #[fg=color,bg=color,bold,dim]
    Style(InlineStyle),
    /// Variable with optional width: {var} or {=12:var}
    Variable { name: String, width: Option<usize> },
    /// Plain text
    Literal(String),
}

/// Parse a tmux-style format string into tokens
/// Supports: #[fg=color,bg=color,bold,dim], {variable}, {=width:variable}, #{variable}
fn parse_tmux_format(format: &str) -> Vec<FormatToken> {
    let mut tokens = Vec::new();
    let mut chars = format.chars().peekable();
    let mut literal = String::new();

    while let Some(ch) = chars.next() {
        if ch == '#' {
            match chars.peek() {
                Some('[') => {
                    // Flush literal
                    if !literal.is_empty() {
                        tokens.push(FormatToken::Literal(std::mem::take(&mut literal)));
                    }
                    chars.next(); // consume '['
                    // Parse style directive until ']'
                    let mut style_str = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == ']' {
                            chars.next();
                            break;
                        }
                        style_str.push(chars.next().unwrap());
                    }
                    tokens.push(FormatToken::Style(parse_style_directive(&style_str)));
                }
                Some('{') => {
                    // Flush literal
                    if !literal.is_empty() {
                        tokens.push(FormatToken::Literal(std::mem::take(&mut literal)));
                    }
                    chars.next(); // consume '{'
                    let var_token = parse_variable(&mut chars);
                    tokens.push(var_token);
                }
                _ => {
                    literal.push(ch);
                }
            }
        } else if ch == '{' {
            // Flush literal
            if !literal.is_empty() {
                tokens.push(FormatToken::Literal(std::mem::take(&mut literal)));
            }
            let var_token = parse_variable(&mut chars);
            tokens.push(var_token);
        } else {
            literal.push(ch);
        }
    }

    if !literal.is_empty() {
        tokens.push(FormatToken::Literal(literal));
    }

    tokens
}

/// Parse style directive content: "fg=color,bg=color,bold,dim"
fn parse_style_directive(content: &str) -> InlineStyle {
    let mut style = InlineStyle::default();

    for part in content.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some(color_str) = part.strip_prefix("fg=") {
            style.fg = parse_color_spec(color_str);
        } else if let Some(color_str) = part.strip_prefix("bg=") {
            style.bg = parse_color_spec(color_str);
        } else if part == "bold" {
            style.bold = true;
        } else if part == "dim" {
            style.dim = true;
        } else if part == "fill" {
            style.fill = true;
        } else if part == "default" || part == "none" || part == "reset" {
            style = InlineStyle::default();
        }
    }

    style
}

/// Parse variable content after '{': "var}" or "=12:var}"
fn parse_variable(chars: &mut std::iter::Peekable<std::str::Chars>) -> FormatToken {
    let mut content = String::new();
    while let Some(&c) = chars.peek() {
        if c == '}' {
            chars.next();
            break;
        }
        content.push(chars.next().unwrap());
    }

    // Check for width specifier: =12:varname
    if let Some(rest) = content.strip_prefix('=')
        && let Some(colon_pos) = rest.find(':')
    {
        let width_str = &rest[..colon_pos];
        let var_name = &rest[colon_pos + 1..];
        if let Ok(width) = width_str.parse::<usize>() {
            return FormatToken::Variable {
                name: var_name.to_string(),
                width: Some(width),
            };
        }
    }

    FormatToken::Variable {
        name: content,
        width: None,
    }
}

/// Parse a styled string like "#[fg=240]│" into StyledText
fn parse_styled_string(s: &str) -> StyledText {
    let tokens = parse_tmux_format(s);
    let mut result = StyledText::new();
    let mut current_style = InlineStyle::default();

    for token in tokens {
        match token {
            FormatToken::Style(style) => {
                current_style = style;
            }
            FormatToken::Literal(text) => {
                result.push(text, current_style.clone());
            }
            FormatToken::Variable { name, .. } => {
                // Variables in border strings are not expanded, treat as literal
                result.push(format!("{{{}}}", name), current_style.clone());
            }
        }
    }

    result
}

// ========== CONFIGURATION ==========

/// Styling configuration for tab labels
#[derive(Clone)]
struct StyleConfig {
    format: String,
    format_active: String,
    overflow_above: String,
    overflow_below: String,
    indicator_active: String,
    indicator_fullscreen: String,
    indicator_sync: String,
    padding_top: usize,
    border: String,
    max_name_length: usize,
    start_index: usize,
    activity_format: String,
    // ----- agent-state presentation (all overridable via plugin config) -----
    /// Rows per tab box (>= 2). Default 3 gives a rounded box with one content row.
    tab_height: usize,
    // State fill colours; None => derive from the Zellij theme.
    color_working: Option<ColorSpec>,
    color_waiting: Option<ColorSpec>,
    color_done: Option<ColorSpec>,
    color_error: Option<ColorSpec>,
    anim_working: Anim,
    anim_waiting: Anim,
    anim_done: Anim,
    anim_error: Anim,
    /// Timer tick (ms) driving flash/scroll animation.
    anim_interval_ms: u64,
    /// Aggregation order, highest priority first. Default: error > waiting > working > done > idle.
    state_priority: Vec<AgentState>,
    /// Idle tab border colours. None => derive from the Zellij theme's frame colours
    /// (focused/unfocused), i.e. the user's base config.
    color_active_border: Option<ColorSpec>,
    color_inactive_border: Option<ColorSpec>,
    /// Channels the agent/action state is shown on (default fill) and the focus
    /// indicator (default border). Accept: fill/border/glyph/both/all/none.
    state_style: Channels,
    focus_style: Channels,
    /// The glyph drawn before the name when the "glyph" channel is on (default "●").
    state_glyph: String,
}

impl StyleConfig {
    fn anim_for(&self, s: AgentState) -> Anim {
        match s {
            AgentState::Working => self.anim_working,
            AgentState::Waiting => self.anim_waiting,
            AgentState::Done => self.anim_done,
            AgentState::Error => self.anim_error,
            AgentState::Idle => Anim::None,
        }
    }
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            format: "{index}:{name}".to_string(),
            format_active: "{index}:{name} {indicators}".to_string(),
            overflow_above: "  ^ +{count}".to_string(),
            overflow_below: "  v +{count}".to_string(),
            indicator_active: "*".to_string(),
            indicator_fullscreen: "Z".to_string(),
            indicator_sync: "S".to_string(),
            max_name_length: 20,
            padding_top: 0,
            border: String::new(),
            start_index: 1,
            activity_format: "#[fg=dim]{activity}".to_string(),
            tab_height: 3,
            color_working: None,
            color_waiting: None,
            color_done: None,
            color_error: None,
            anim_working: Anim::Scroll,
            anim_waiting: Anim::Flash,
            anim_done: Anim::Solid,
            anim_error: Anim::Flash,
            anim_interval_ms: 500,
            state_priority: vec![
                AgentState::Error,
                AgentState::Waiting,
                AgentState::Working,
                AgentState::Done,
                AgentState::Idle,
            ],
            color_active_border: None,
            color_inactive_border: None,
            state_style: Channels {
                fill: true,
                border: false,
                glyph: false,
            },
            focus_style: Channels {
                fill: false,
                border: true,
                glyph: false,
            },
            state_glyph: "●".to_string(),
        }
    }
}

// ========== PLUGIN STATE ==========

#[derive(Default)]
struct State {
    tabs: Vec<TabInfo>,
    active_tab_idx: usize,
    mode_info: ModeInfo,
    pane_manifest: PaneManifest,
    style: StyleConfig,
    last_rows: usize,
    permissions_granted: bool,
    is_selectable: bool,
    pending_events: Vec<Event>,
    activity: BTreeMap<String, activity::Activity>,
    own_session: String,
    /// Latest reported agent state per pane id (from the `agent_state` pipe).
    pane_agents: BTreeMap<u32, PaneAgent>,
    /// Animation frame counter, incremented on each Timer tick.
    frame: u64,
    /// Whether a timer is currently armed (avoids stacking timeouts).
    timer_running: bool,
    /// Row ranges each visible tab occupies, rebuilt every render: (start, end_exclusive, tab_index).
    tab_rows: Vec<(usize, usize, usize)>,
    /// Set when config is invalid (e.g. state_style/focus_style conflict); shown as a banner.
    config_error: Option<String>,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // Parse style configuration
        if let Some(v) = configuration.get("format") {
            self.style.format = v.clone();
        }
        if let Some(v) = configuration.get("format_active") {
            self.style.format_active = v.clone();
        }
        if let Some(v) = configuration.get("overflow_above") {
            self.style.overflow_above = v.clone();
        }
        if let Some(v) = configuration.get("overflow_below") {
            self.style.overflow_below = v.clone();
        }
        if let Some(v) = configuration.get("indicator_active") {
            self.style.indicator_active = v.clone();
        }
        if let Some(v) = configuration.get("indicator_fullscreen") {
            self.style.indicator_fullscreen = v.clone();
        }
        if let Some(v) = configuration.get("indicator_sync") {
            self.style.indicator_sync = v.clone();
        }
        if let Some(v) = configuration.get("max_name_length")
            && let Ok(n) = v.parse::<usize>()
        {
            self.style.max_name_length = n;
        }
        if let Some(v) = configuration.get("padding_top")
            && let Ok(n) = v.parse::<usize>()
        {
            self.style.padding_top = n;
        }
        if let Some(v) = configuration.get("border") {
            self.style.border = v.clone();
        } else if let Some(v) = configuration.get("border_char") {
            self.style.border = v.clone();
        }
        if let Some(v) = configuration.get("start_index")
            && let Ok(n) = v.parse::<usize>()
        {
            self.style.start_index = n;
        }
        if let Some(v) = configuration.get("activity_format") {
            self.style.activity_format = v.clone();
        }

        // ----- agent-state presentation config -----
        if let Some(v) = configuration.get("tab_height")
            && let Ok(n) = v.parse::<usize>()
        {
            self.style.tab_height = n.max(2);
        }
        if let Some(v) = configuration.get("color_working") {
            self.style.color_working = Some(parse_color_spec(v));
        }
        if let Some(v) = configuration.get("color_waiting") {
            self.style.color_waiting = Some(parse_color_spec(v));
        }
        if let Some(v) = configuration.get("color_done") {
            self.style.color_done = Some(parse_color_spec(v));
        }
        if let Some(v) = configuration.get("color_error") {
            self.style.color_error = Some(parse_color_spec(v));
        }
        if let Some(v) = configuration.get("anim_working") {
            self.style.anim_working = Anim::parse(v);
        }
        if let Some(v) = configuration.get("anim_waiting") {
            self.style.anim_waiting = Anim::parse(v);
        }
        if let Some(v) = configuration.get("anim_done") {
            self.style.anim_done = Anim::parse(v);
        }
        if let Some(v) = configuration.get("anim_error") {
            self.style.anim_error = Anim::parse(v);
        }
        if let Some(v) = configuration.get("anim_interval_ms")
            && let Ok(n) = v.parse::<u64>()
        {
            self.style.anim_interval_ms = n.max(50);
        }
        if let Some(v) = configuration.get("state_priority") {
            let order: Vec<AgentState> = v.split_whitespace().map(AgentState::parse).collect();
            if !order.is_empty() {
                self.style.state_priority = order;
            }
        }
        if let Some(v) = configuration.get("color_active_border") {
            self.style.color_active_border = Some(parse_color_spec(v));
        }
        if let Some(v) = configuration.get("color_inactive_border") {
            self.style.color_inactive_border = Some(parse_color_spec(v));
        }
        if let Some(v) = configuration.get("state_style") {
            self.style.state_style = Channels::parse(v);
        }
        if let Some(v) = configuration.get("focus_style") {
            self.style.focus_style = Channels::parse(v);
        }
        if let Some(v) = configuration.get("state_glyph") {
            self.style.state_glyph = v.clone();
        }
        // A visualisation conflict: state and focus cannot share the same channel.
        let s = self.style.state_style;
        let f = self.style.focus_style;
        self.config_error = if s.border && f.border {
            Some(
                "state_style & focus_style both use the BORDER — set one to fill/glyph/none".into(),
            )
        } else if s.fill && f.fill {
            Some(
                "state_style & focus_style both use the FILL — set one to border/glyph/none".into(),
            )
        } else if s.glyph && f.glyph {
            Some(
                "state_style & focus_style both use the GLYPH — set one to fill/border/none".into(),
            )
        } else {
            None
        };

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            // Needed to broadcast a state-sync request/response via `zellij pipe`
            // so a newly-created tab's plugin copy can catch up on existing state.
            PermissionType::RunCommands,
        ]);

        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::ModeUpdate,
            EventType::Mouse,
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::Timer,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;

        if let Event::PermissionRequestResult(status) = event {
            if status == PermissionStatus::Granted {
                self.permissions_granted = true;
                self.is_selectable = false;
                set_selectable(false);
                // This copy just loaded (new tab): ask peers to resend their state.
                self.request_state_sync();

                while !self.pending_events.is_empty() {
                    let cached_event = self.pending_events.remove(0);
                    self.update(cached_event);
                }
                should_render = true;
            }
            return should_render;
        }

        if !self.permissions_granted {
            self.pending_events.push(event);
            return false;
        }

        match event {
            Event::PermissionRequestResult(_) => {}
            Event::ModeUpdate(mode_info) => {
                if self.mode_info != mode_info {
                    should_render = true;
                }
                self.mode_info = mode_info;
            }
            Event::TabUpdate(tabs) => {
                let active_tab_index = tabs.iter().position(|t| t.active).unwrap_or(0);
                let active_tab_idx = active_tab_index + 1;
                if self.active_tab_idx != active_tab_idx || self.tabs != tabs {
                    should_render = true;
                }
                self.active_tab_idx = active_tab_idx;
                self.tabs = tabs;
            }
            Event::PaneUpdate(pane_manifest) => {
                self.pane_manifest = pane_manifest;
                // NOTE: do NOT prune pane_agents against this manifest. Zellij can send
                // a PaneUpdate that omits background tabs' panes, and pruning would then
                // permanently delete their state (e.g. an errored tab loses its red on a
                // tab switch). A stale entry for a truly-closed pane is harmless — it
                // simply won't match any live tab in tab_agent().
                self.arm_timer();
                should_render = true;
            }
            Event::Timer(_) => {
                self.timer_running = false;
                self.frame = self.frame.wrapping_add(1);
                if self.any_animated() {
                    self.arm_timer();
                    should_render = true;
                }
            }
            Event::Mouse(me) => match me {
                Mouse::LeftClick(row, _col) => {
                    if let Some(idx) = self.get_tab_at_row(row as usize) {
                        switch_tab_to(idx as u32);
                    }
                    // Refresh so the click-row map stays current after any click.
                    should_render = true;
                }
                Mouse::ScrollUp(_) => {
                    let prev_tab = max(self.active_tab_idx.saturating_sub(1), 1);
                    switch_tab_to(prev_tab as u32);
                }
                Mouse::ScrollDown(_) => {
                    let next_tab = min(self.active_tab_idx + 1, self.tabs.len());
                    switch_tab_to(next_tab as u32);
                }
                _ => {}
            },
            Event::SessionUpdate(sessions, _) => {
                if let Some(s) = sessions.iter().find(|s| s.is_current_session)
                    && self.own_session != s.name
                {
                    self.own_session = s.name.clone();
                    should_render = true;
                }
            }
            _ => {}
        }
        should_render
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        match pipe_message.name.as_str() {
            "set_selectable" => {
                match pipe_message.payload.as_deref() {
                    Some("true") => {
                        self.is_selectable = true;
                        set_selectable(true);
                    }
                    Some("false") => {
                        self.is_selectable = false;
                        set_selectable(false);
                    }
                    _ => {}
                }
                false
            }
            "toggle_selectable" => {
                self.is_selectable = !self.is_selectable;
                set_selectable(self.is_selectable);
                false
            }
            "activity" => {
                if let Some(payload) = pipe_message.payload.as_deref()
                    && let Some((zsession, name, act)) = activity::parse_activity(payload)
                {
                    self.activity
                        .insert(format!("{}\u{1}{}", zsession, name), act);
                    return true;
                }
                false
            }
            // Agent state protocol: "<pane_id>\x1f<state>\x1f<agent>\x1f<label>"
            "agent_state" => {
                if let Some(payload) = pipe_message.payload.as_deref() {
                    self.apply_state_payload(payload);
                    self.arm_timer();
                    return true;
                }
                false
            }
            // A newly-loaded copy (new tab) asks peers to resend their state.
            "zat_sync_request" => {
                self.broadcast_state();
                false
            }
            // A peer's full state dump (newline-separated agent_state payloads).
            "zat_sync_state" => {
                if let Some(payload) = pipe_message.payload.as_deref() {
                    for line in payload.split('\n') {
                        self.apply_state_payload(line);
                    }
                    self.arm_timer();
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.last_rows = rows;

        if !self.permissions_granted {
            return;
        }
        if let Some(err) = self.config_error.clone() {
            self.render_error(rows, cols, &err);
            return;
        }
        if self.tabs.is_empty() {
            return;
        }

        self.render_vertical(rows, cols);
    }
}

impl State {
    /// Arm the animation timer if not already running and something needs animating.
    fn arm_timer(&mut self) {
        if !self.timer_running && self.any_animated() {
            self.timer_running = true;
            set_timeout(self.style.anim_interval_ms as f64 / 1000.0);
        }
    }

    /// Parse one "<pane_id>\x1f<state>\x1f<agent>\x1f<label>" payload into pane_agents.
    fn apply_state_payload(&mut self, payload: &str) {
        let parts: Vec<&str> = payload.split('\u{1f}').collect();
        if parts.len() >= 2
            && let Ok(pid) = parts[0].trim().parse::<u32>()
        {
            let state = AgentState::parse(parts[1]);
            if state == AgentState::Idle {
                self.pane_agents.remove(&pid);
            } else {
                self.pane_agents.insert(
                    pid,
                    PaneAgent {
                        state,
                        agent: parts.get(2).map(|s| s.to_string()).unwrap_or_default(),
                        label: parts.get(3).map(|s| s.to_string()).unwrap_or_default(),
                    },
                );
            }
        }
    }

    /// Ask peer copies (other tabs) to resend their state — broadcast via `zellij pipe`.
    fn request_state_sync(&self) {
        run_command(
            &["zellij", "pipe", "--name", "zat_sync_request"],
            BTreeMap::new(),
        );
    }

    /// Broadcast this copy's full state so late-joining copies can catch up.
    fn broadcast_state(&self) {
        if self.pane_agents.is_empty() {
            return;
        }
        let payload = self
            .pane_agents
            .iter()
            .map(|(id, pa)| {
                format!(
                    "{}\u{1f}{}\u{1f}{}\u{1f}{}",
                    id,
                    pa.state.key(),
                    pa.agent,
                    pa.label
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        run_command(
            &["zellij", "pipe", "--name", "zat_sync_state", "--", &payload],
            BTreeMap::new(),
        );
    }

    /// Any pane (adapter-reported OR a running/finished command pane) animated?
    fn any_animated(&self) -> bool {
        self.pane_manifest
            .panes
            .values()
            .flatten()
            .filter(|p| !p.is_plugin)
            .filter_map(|p| self.pane_state(p))
            .any(|(state, _)| self.style.anim_for(state).is_animated())
    }

    /// State for a single pane. Adapter-reported state (via the pipe) wins; otherwise
    /// fall back to native detection for command panes (no adapter/shell needed):
    /// running command → working, exited/held → done or error by exit status.
    fn pane_state(&self, pane: &PaneInfo) -> Option<(AgentState, String)> {
        if let Some(pa) = self.pane_agents.get(&pane.id) {
            return Some((pa.state, pa.label.clone()));
        }
        if let Some(cmd) = &pane.terminal_command {
            let state = if pane.exited || pane.is_held {
                match pane.exit_status {
                    Some(0) => AgentState::Done,
                    Some(_) => AgentState::Error,
                    None => AgentState::Done,
                }
            } else {
                AgentState::Working
            };
            return Some((state, cmd.clone()));
        }
        None
    }

    /// Priority index of a state (lower = higher priority / less complete).
    fn prio(&self, s: AgentState) -> usize {
        self.style
            .state_priority
            .iter()
            .position(|&x| x == s)
            .unwrap_or(usize::MAX)
    }

    /// Idle border colour for the active tab (config override, else theme focused frame).
    fn border_active(&self) -> ColorSpec {
        self.style
            .color_active_border
            .unwrap_or_else(|| palette_to_spec(self.mode_info.style.colors.frame_selected.base))
    }

    /// Idle border colour for inactive tabs (config override, else theme unfocused frame).
    fn border_inactive(&self) -> ColorSpec {
        self.style.color_inactive_border.unwrap_or_else(|| {
            self.mode_info
                .style
                .colors
                .frame_unselected
                .map(|d| palette_to_spec(d.base))
                .unwrap_or(ColorSpec::EightBit(240))
        })
    }

    /// Fill colour for a state: config override, else derived from the Zellij theme
    /// (success colour for working/done, error colour for error, themed yellow for waiting).
    fn state_color(&self, s: AgentState) -> ColorSpec {
        let cfg = match s {
            AgentState::Working => self.style.color_working,
            AgentState::Waiting => self.style.color_waiting,
            AgentState::Done => self.style.color_done,
            AgentState::Error => self.style.color_error,
            AgentState::Idle => return ColorSpec::Default,
        };
        cfg.unwrap_or_else(|| {
            let c = &self.mode_info.style.colors;
            match s {
                AgentState::Working | AgentState::Done => palette_to_spec(c.exit_code_success.base),
                AgentState::Error => palette_to_spec(c.exit_code_error.base),
                AgentState::Waiting => ColorSpec::EightBit(3), // themed yellow (no theme "warning")
                AgentState::Idle => ColorSpec::Default,
            }
        })
    }

    /// Aggregate agent state across a tab's panes: least-complete / highest-priority
    /// wins. Returns the winning state and the label from the winning pane (if any).
    fn tab_agent(&self, tab_position: usize) -> (AgentState, Option<String>) {
        let mut best: Option<(AgentState, String)> = None;
        if let Some(panes) = self.pane_manifest.panes.get(&tab_position) {
            for pane in panes {
                if pane.is_plugin {
                    continue;
                }
                if let Some((state, label)) = self.pane_state(pane) {
                    let better = match &best {
                        None => true,
                        Some((bs, _)) => self.prio(state) < self.prio(*bs),
                    };
                    if better {
                        best = Some((state, label));
                    }
                }
            }
        }
        match best {
            Some((s, label)) => (s, if label.is_empty() { None } else { Some(label) }),
            None => (AgentState::Idle, None),
        }
    }

    /// Display name for a tab: its explicit name, else the focused pane's title.
    fn tab_display_name(&self, tab: &TabInfo) -> String {
        if !tab.name.starts_with("Tab #") && !tab.name.is_empty() {
            return tab.name.clone();
        }
        self.get_focused_pane_title(tab.position)
            .unwrap_or_else(|| "…".to_string())
    }

    /// Render one rounded tab box of `height` rows (each `cols` display-columns wide).
    fn render_box(
        &self,
        index: usize,
        is_active: bool,
        state: AgentState,
        name: &str,
        cols: usize,
        height: usize,
    ) -> Vec<String> {
        let h = height.max(2);
        if cols < 3 {
            return vec![" ".repeat(cols); h];
        }
        let inner = cols - 2;
        let reset = "\x1b[0m";
        let dim = ColorSpec::EightBit(238);

        let fc = if is_active {
            self.border_active()
        } else {
            self.border_inactive()
        };
        let sc = self.state_color(state);
        let anim = self.style.anim_for(state);
        let has_state = state != AgentState::Idle;

        // ---- border: state (if routed here) wins, else focus, else muted ----
        let border_state = has_state && self.style.state_style.border;
        let border_color = if border_state {
            match anim {
                Anim::Flash if self.frame % 2 != 0 => dim,
                _ => sc,
            }
        } else if self.style.focus_style.border {
            fc
        } else {
            ColorSpec::EightBit(240)
        };
        let bc = border_color.to_ansi_fg();
        let border_scroll = border_state && anim == Anim::Scroll && inner > 0;

        // ---- fill: state (if routed here), else focus-fill on the active tab ----
        let fill_state = has_state && self.style.state_style.fill;
        let fill_focus = !has_state && is_active && self.style.focus_style.fill;
        let (fill_color, fill_on) = if fill_state {
            (sc, !matches!(anim, Anim::Flash) || self.frame % 2 == 0)
        } else if fill_focus {
            (fc, true)
        } else {
            (ColorSpec::Default, false)
        };
        let fill_bg = if fill_on {
            fill_color.to_ansi_bg()
        } else {
            String::new()
        };
        let fill_scroll = fill_state && anim == Anim::Scroll && inner > 0;
        let scroll_pos = if border_scroll || fill_scroll {
            Some((self.frame as usize) % inner)
        } else {
            None
        };

        // ---- glyph: a coloured indicator before the name ----
        let glyph_state = has_state && self.style.state_style.glyph;
        let glyph_focus = !has_state && is_active && self.style.focus_style.glyph;
        let glyph_on = glyph_state || glyph_focus;
        let glyph_fg = if glyph_state { sc } else { fc }.to_ansi_fg();
        let glyph_w = if glyph_on {
            self.style.state_glyph.width()
        } else {
            0
        };

        // Interior text, padded to the inner width (leading glyph or a space).
        let label = format!("{} {}", index, name);
        let mut interior = String::new();
        if glyph_on {
            interior.push_str(&self.style.state_glyph);
            interior.push(' ');
        } else {
            interior.push(' ');
        }
        interior.push_str(&label);
        let mut inner_text = truncate_string(&interior, inner);
        for _ in inner_text.width()..inner {
            inner_text.push(' ');
        }

        // Horizontal border, with a scroll runner when state is routed to the border.
        let hbar = |left: char, right: char| -> String {
            let mut s = String::new();
            s.push_str(&bc);
            s.push(left);
            for i in 0..inner {
                if border_scroll && Some(i) == scroll_pos {
                    s.push_str("\x1b[1m");
                    s.push('━');
                    s.push_str("\x1b[22m");
                } else {
                    s.push('─');
                }
            }
            s.push(right);
            s.push_str(reset);
            s
        };
        let top = hbar('╭', '╮');
        let bottom = hbar('╰', '╯');

        // A content row: side borders + (optionally filled) interior.
        let content_line = |with_content: bool| -> String {
            let mut s = String::new();
            s.push_str(&bc);
            s.push('│');
            s.push_str(reset);
            if fill_on {
                s.push_str(&fill_bg);
            }
            if with_content && is_active {
                s.push_str("\x1b[1m");
            }
            let show_glyph = with_content && glyph_on;
            if show_glyph {
                s.push_str(&glyph_fg);
            }
            let chars: Vec<char> = if with_content {
                inner_text.chars().collect()
            } else {
                vec![' '; inner]
            };
            for (i, ch) in chars.iter().enumerate() {
                if show_glyph && i == glyph_w {
                    s.push_str("\x1b[39m"); // end glyph colour, keep bg/bold
                }
                if fill_scroll && Some(i) == scroll_pos {
                    s.push_str("\x1b[7m"); // reverse => bright moving cell on the fill
                    s.push(*ch);
                    s.push_str("\x1b[27m");
                } else {
                    s.push(*ch);
                }
            }
            s.push_str(reset);
            s.push_str(&bc);
            s.push('│');
            s.push_str(reset);
            s
        };

        let mut lines = Vec::with_capacity(h);
        if h == 2 {
            lines.push(content_line(true));
            lines.push(bottom);
        } else {
            lines.push(top);
            let content_rows = h - 2;
            for i in 0..content_rows {
                lines.push(content_line(i == 0));
            }
            lines.push(bottom);
        }
        lines
    }

    /// Render an obvious red banner across the whole pane (invalid config).
    fn render_error(&self, rows: usize, cols: usize, msg: &str) {
        let bg = ColorSpec::EightBit(196).to_ansi_bg();
        let fg = ColorSpec::EightBit(231).to_ansi_fg();
        let reset = "\x1b[0m";

        // Word-wrap the message to the pane width.
        let mut body: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in msg.split_whitespace() {
            if cur.is_empty() {
                cur = word.to_string();
            } else if cur.width() + 1 + word.width() <= cols {
                cur.push(' ');
                cur.push_str(word);
            } else {
                body.push(std::mem::take(&mut cur));
                cur = word.to_string();
            }
        }
        if !cur.is_empty() {
            body.push(cur);
        }

        let banner = |text: &str| -> String {
            let mut t = truncate_string(text, cols);
            for _ in t.width()..cols {
                t.push(' ');
            }
            format!("{}{}{}{}", bg, fg, t, reset)
        };

        let mut lines: Vec<String> = Vec::with_capacity(rows);
        lines.push(banner(" ⚠ agent-tabs config"));
        for l in &body {
            lines.push(banner(&format!(" {}", l)));
        }
        while lines.len() < rows {
            lines.push(" ".repeat(cols));
        }
        for (i, line) in lines.iter().enumerate() {
            if i + 1 < lines.len() {
                println!("{}", line);
            } else {
                print!("{}", line);
            }
        }
    }

    fn get_focused_pane_title(&self, tab_position: usize) -> Option<String> {
        if let Some(panes) = self.pane_manifest.panes.get(&tab_position) {
            for pane in panes {
                if pane.is_focused && !pane.is_plugin {
                    let title = &pane.title;
                    if title.starts_with("Pane #") || title.starts_with("Tab #") || title.is_empty()
                    {
                        return None;
                    }
                    return Some(title.clone());
                }
            }
        }
        None
    }

    fn expand_overflow_format(&self, format: &str, count: usize) -> String {
        format.replace("{count}", &count.to_string())
    }

    /// Expand a tmux-style format string with tab info, returning styled text
    fn expand_tmux_format(&self, format: &str, tab: &TabInfo, index: usize) -> StyledText {
        let tokens = parse_tmux_format(format);
        let mut result = StyledText::new();
        let mut current_style = InlineStyle::default();

        // Get focused pane title for this tab
        let pane_title = self
            .get_focused_pane_title(tab.position)
            .or_else(|| {
                if !tab.name.starts_with("Tab #") {
                    Some(tab.name.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "...".to_string());

        // Build indicators string
        let mut indicators = String::new();
        if tab.is_fullscreen_active {
            indicators.push_str(&self.style.indicator_fullscreen);
        }
        if tab.is_sync_panes_active {
            indicators.push_str(&self.style.indicator_sync);
        }
        if tab.active {
            indicators.push_str(&self.style.indicator_active);
        }

        for token in tokens {
            match token {
                FormatToken::Style(style) => {
                    current_style = style;
                }
                FormatToken::Variable { name, width } => {
                    let value = match name.as_str() {
                        "index" | "i" => index.to_string(),
                        "name" | "n" => {
                            if tab.active
                                && self.mode_info.mode == InputMode::RenameTab
                                && tab.name.is_empty()
                            {
                                "Enter name...".to_string()
                            } else if !tab.name.starts_with("Tab #") && !tab.name.is_empty() {
                                tab.name.clone()
                            } else {
                                pane_title.clone()
                            }
                        }
                        "title" | "t" | "pane_title" => pane_title.clone(),
                        "indicators" => indicators.clone(),
                        "fullscreen" => {
                            if tab.is_fullscreen_active {
                                self.style.indicator_fullscreen.clone()
                            } else {
                                String::new()
                            }
                        }
                        "sync" => {
                            if tab.is_sync_panes_active {
                                self.style.indicator_sync.clone()
                            } else {
                                String::new()
                            }
                        }
                        "active" => {
                            if tab.active {
                                self.style.indicator_active.clone()
                            } else {
                                String::new()
                            }
                        }
                        _ => format!("{{{}}}", name),
                    };

                    let text = if let Some(w) = width {
                        truncate_string(&value, w)
                    } else {
                        truncate_string(&value, self.style.max_name_length)
                    };

                    result.push(text, current_style.clone());
                }
                FormatToken::Literal(text) => {
                    result.push(text, current_style.clone());
                }
            }
        }

        result
    }

    /// Build a complete line with content, padding, and border
    fn build_line(&self, content: &StyledText, cols: usize, is_selected: bool) -> String {
        let border = parse_styled_string(&self.style.border);
        let border_width = border.display_width();

        let effective_cols = cols.saturating_sub(border_width);

        // Truncate content if it exceeds available width to prevent wrapping
        let content = content.truncate(effective_cols);
        let content_width = content.display_width();
        let padding_needed = effective_cols.saturating_sub(content_width);

        let mut line = String::new();

        // Check if any segment has fill attribute - fills entire row with bg color
        let has_fill = is_selected && content.segments.iter().any(|s| s.style.fill);

        if has_fill {
            // Fill mode: use reverse video with swapped colors so bg fills the row
            // User writes #[bg=236,fill] -> we swap to fg=236 -> reverse makes displayed bg=236
            line.push_str("\x1b[7m");

            for segment in &content.segments {
                // Swap fg and bg for reverse video
                let mut swapped_style = segment.style.clone();
                std::mem::swap(&mut swapped_style.fg, &mut swapped_style.bg);
                swapped_style.fill = false; // Don't need fill flag in output

                if swapped_style.has_any_style() {
                    line.push_str("\x1b[0m\x1b[7m"); // Reset and re-apply reverse
                    line.push_str(&swapped_style.to_ansi());
                }
                line.push_str(&segment.text);
            }

            if padding_needed > 0 {
                line.push_str(&" ".repeat(padding_needed));
            }

            line.push_str("\x1b[0m");
        } else {
            // Normal rendering - bg colors only apply to text, not padding
            line.push_str(&content.to_ansi());

            if padding_needed > 0 {
                line.push_str(&" ".repeat(padding_needed));
            }
        }

        // Add border (not affected by selection)
        if border_width > 0 {
            line.push_str(&border.to_ansi());
        }

        line
    }

    /// Build a line with just the border (for empty rows)
    fn build_empty_line(&self, cols: usize) -> String {
        let border = parse_styled_string(&self.style.border);
        let border_width = border.display_width();

        if border_width == 0 {
            return " ".repeat(cols);
        }

        let effective_cols = cols.saturating_sub(border_width);
        let mut line = " ".repeat(effective_cols);
        line.push_str(&border.to_ansi());
        line
    }

    fn render_vertical(&mut self, rows: usize, cols: usize) {
        self.tab_rows.clear();

        let top_padding = self.style.padding_top;
        let tab_h = self.style.tab_height.max(2);
        let tab_count = self.tabs.len();
        let active_index = self.active_tab_idx.saturating_sub(1);

        // How many whole tab boxes fit in the available height.
        let available_rows = rows.saturating_sub(top_padding);
        let max_visible = (available_rows / tab_h).max(1);

        let (start_index, end_index, tabs_above, tabs_below) =
            calculate_visible_range(tab_count, max_visible, active_index);

        let mut lines: Vec<String> = Vec::with_capacity(rows);

        for _ in 0..top_padding {
            lines.push(" ".repeat(cols));
        }

        if tabs_above > 0 {
            let text = self.expand_overflow_format(&self.style.overflow_above, tabs_above);
            lines.push(self.build_line(&parse_styled_string(&text), cols, false));
        }

        for i in start_index..end_index {
            if lines.len() >= rows {
                break;
            }
            if let Some(tab) = self.tabs.get(i).cloned() {
                let (state, label) = self.tab_agent(tab.position);
                let name = label.unwrap_or_else(|| self.tab_display_name(&tab));
                let start_row = lines.len();
                for bl in self.render_box(
                    i + self.style.start_index,
                    tab.active,
                    state,
                    &name,
                    cols,
                    tab_h,
                ) {
                    if lines.len() >= rows {
                        break;
                    }
                    lines.push(bl);
                }
                // Map the rows this box occupies back to a 1-based tab position for clicks.
                self.tab_rows.push((start_row, lines.len(), i + 1));
            }
        }

        if tabs_below > 0 && lines.len() < rows {
            let text = self.expand_overflow_format(&self.style.overflow_below, tabs_below);
            lines.push(self.build_line(&parse_styled_string(&text), cols, false));
        }

        while lines.len() < rows {
            lines.push(" ".repeat(cols));
        }

        for (i, line) in lines.iter().enumerate() {
            if i + 1 < lines.len() {
                println!("{}\x1b[m", line);
            } else {
                print!("{}\x1b[m", line);
            }
        }
    }

    fn get_tab_at_row(&self, row: usize) -> Option<usize> {
        self.tab_rows
            .iter()
            .find(|&&(start, end, _)| row >= start && row < end)
            .map(|&(_, _, tab_index)| tab_index)
    }
}

fn calculate_visible_range(
    tab_count: usize,
    available_rows: usize,
    active_index: usize,
) -> (usize, usize, usize, usize) {
    if tab_count == 0 {
        return (0, 0, 0, 0);
    }

    if tab_count <= available_rows {
        return (0, tab_count, 0, 0);
    }

    let max_visible = available_rows.saturating_sub(2);
    if max_visible == 0 {
        return (0, 0, tab_count, 0);
    }

    let mut start_index = active_index;
    let mut end_index = active_index + 1;
    let mut room_left = max_visible.saturating_sub(1);
    let mut alternate = false;

    while room_left > 0 {
        if !alternate && start_index > 0 {
            start_index -= 1;
            room_left -= 1;
        } else if alternate && end_index < tab_count {
            end_index += 1;
            room_left -= 1;
        } else if start_index > 0 {
            start_index -= 1;
            room_left -= 1;
        } else if end_index < tab_count {
            end_index += 1;
            room_left -= 1;
        } else {
            break;
        }
        alternate = !alternate;
    }

    (
        start_index,
        end_index,
        start_index,
        tab_count.saturating_sub(end_index),
    )
}

fn norm_session_name(s: &str) -> String {
    let t = s.trim_start();
    let mut chars = t.chars();
    if let Some(first) = chars.clone().next()
        && !first.is_alphanumeric()
    {
        return chars
            .by_ref()
            .skip(1)
            .collect::<String>()
            .trim_start()
            .to_string();
    }
    t.to_string()
}

fn truncate_string(s: &str, max_width: usize) -> String {
    if s.width() <= max_width {
        return s.to_string();
    }

    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let mut truncated = String::new();
    let mut width = 0;
    for ch in s.chars() {
        let ch_width = ch.to_string().width();
        if width + ch_width + 3 > max_width {
            truncated.push_str("...");
            break;
        }
        truncated.push(ch);
        width += ch_width;
    }
    truncated
}
