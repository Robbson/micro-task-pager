using System;
using System.Collections.Generic;
using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using MicroTaskPager.Config;

namespace MicroTaskPager.GUI
{
    // Settings dialog built entirely in code. All rows are created through a few helpers so that
    // layout, revert buttons, "Defaults" handling and live preview behave identically everywhere.
    public partial class AppSettingsWindow : Form
    {
        private const int PlacementGap = 8;
        private const int MinPagerContentHeight = 16;

        private static readonly int[] _iconSizes = { 16, 24, 32, 48, 64 };

        private readonly AppSettings _defaults = new AppSettings();
        private readonly AppSettings _settingsBefore;
        private readonly AppSettings _working;
        private readonly List<RowBinding> _rows = new List<RowBinding>();
        private readonly ToolTip _toolTip = new ToolTip();
        private readonly Timer _previewTimer = new Timer { Interval = 80 };

        private TabControl _tabs;
        private TextBox _primaryDelayBox;
        private TextBox _secondaryDelayBox;
        private Rectangle _placementOwnerBounds;
        private bool _hasPlacementOwner;
        private bool _initializing = true;
        private bool _suppressEvents;
        private bool _closing;

        public AppSettingsWindow(AppSettings settings, Rectangle ownerBounds)
        {
            _settingsBefore = AppSettings.Clone(settings);
            _working = AppSettings.Clone(settings);

            InitializeComponent();

            _placementOwnerBounds = ownerBounds;
            _hasPlacementOwner = true;

            _previewTimer.Tick += (s, e) =>
            {
                _previewTimer.Stop();
                FlushPreview();
            };

            _initializing = false;
            RefreshRevertButtons();
        }

        public event Action<AppSettings> PreviewChanged;

        public AppSettings Settings { get; private set; }

        protected override void OnLoad(EventArgs e)
        {
            base.OnLoad(e);

            if (_hasPlacementOwner)
            {
                Location = FindPlacement(_placementOwnerBounds, Size);
            }
        }

        protected override void OnFormClosing(FormClosingEventArgs e)
        {
            // A queued preview must not be applied after the dialog was cancelled.
            _closing = true;
            _previewTimer.Stop();
            base.OnFormClosing(e);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                _previewTimer.Dispose();
                _toolTip.Dispose();
            }

            base.Dispose(disposing);
        }

        // ------------------------------------------------------------------------------------
        // Static helpers
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// Appends an auto-sized row to the given table and returns the new zero-based row index.
        /// </summary>
        /// <returns>The index of the appended row.</returns>
        private static int AddTableRow(TableLayoutPanel table)
        {
            table.RowStyles.Add(new RowStyle(SizeType.AutoSize));
            table.RowCount = table.RowStyles.Count;
            return table.RowCount - 1;
        }

        private static int IconSizeIndex(int size)
        {
            int bestIndex = 0;
            for (int i = 1; i < _iconSizes.Length; i++)
            {
                if (Math.Abs(_iconSizes[i] - size) < Math.Abs(_iconSizes[bestIndex] - size))
                {
                    bestIndex = i;
                }
            }

            return bestIndex;
        }

        // The combo box lists "No" last even though it is the default and has the value 0.
        private static int MinimizedVisibilityIndex(MinimizedWindowVisibility visibility)
        {
            switch (visibility)
            {
                case MinimizedWindowVisibility.Yes:
                    return 0;
                case MinimizedWindowVisibility.AfterOthers:
                    return 1;
                default:
                    return 2;
            }
        }

        private static MinimizedWindowVisibility MinimizedVisibilityFromIndex(int index)
        {
            switch (index)
            {
                case 0:
                    return MinimizedWindowVisibility.Yes;
                case 1:
                    return MinimizedWindowVisibility.AfterOthers;
                default:
                    return MinimizedWindowVisibility.No;
            }
        }

        // ------------------------------------------------------------------------------------
        // UI construction
        // ------------------------------------------------------------------------------------

        private void InitializeComponent()
        {
            SuspendLayout();

            // Pixel layout is authored at 96 DPI; let WinForms scale it like AboutDialog does.
            AutoScaleDimensions = new SizeF(96F, 96F);
            AutoScaleMode = AutoScaleMode.Dpi;

            Text = "Micro Task Pager Settings";
            ClientSize = new Size(520, 570);
            StartPosition = FormStartPosition.Manual;
            FormBorderStyle = FormBorderStyle.FixedDialog;
            MaximizeBox = false;
            MinimizeBox = false;
            ShowInTaskbar = false;

            // The pager itself is topmost. Keeping the modal settings dialog topmost as well
            // prevents the pager from being raised above it and stealing the foreground while
            // the preview is updated.
            TopMost = true;
            Padding = new Padding(10, 10, 10, 0);

            // ---- Add tabs and their pages ----
            _tabs = new TabControl
            {
                Dock = DockStyle.Fill,
            };

            _tabs.TabPages.Add(CreateLayoutTab());
            _tabs.TabPages.Add(CreateThemeTab());
            _tabs.TabPages.Add(CreateMiscTab());

            // ---- Add generic dialog buttons at bottom ----
            var buttons = new FlowLayoutPanel
            {
                Dock = DockStyle.Bottom,
                FlowDirection = FlowDirection.RightToLeft,
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                WrapContents = false,
                Padding = new Padding(0, 4, 0, 4),
            };

            var cancel = new Button { Text = "Cancel", Width = 100, Height = 28, Margin = new Padding(8, 0, 0, 0) };
            cancel.Click += Cancel_Click;

            var save = new Button { Text = "Save", Width = 100, Height = 28, Margin = new Padding(8, 0, 0, 0) };
            save.Click += Save_Click;

            var defaults = new Button { Text = "Defaults", Width = 100, Height = 28, Margin = new Padding(0) };
            _toolTip.SetToolTip(defaults, "Reset for current tab only");
            defaults.Click += Defaults_Click;

            buttons.Controls.Add(cancel);
            buttons.Controls.Add(save);
            buttons.Controls.Add(defaults);

            AcceptButton = save;
            CancelButton = cancel;

            Controls.Add(_tabs);
            Controls.Add(buttons);

            ResumeLayout(false);
        }

        private TabPage CreateLayoutTab()
        {
            TabPage layoutPage = CreateTabPage("Layout");

            Render(
                layoutPage,
                BeginGroup(layoutPage, "Rendering"),
                ComboRow(
                    "View Mode Default",
                    "Only affects newly created desktops and their screens.",
                    new[] { "Preview", "MicroTaskbar" },
                    s => (int)s.ViewModeDefault,
                    (s, v) => s.ViewModeDefault = (ViewMode)v),
                NumericRow(
                    "Pager Height",
                    "The pager's width is automatically calculated from this value and from the size of all available screen space as well as the amount of virtual desktops.",
                    MinPagerContentHeight,
                    9999,
                    s => s.PagerHeight,
                    (s, v) => s.PagerHeight = v,
                    value => Math.Max(value, _working.IconPaddingY + MinPagerContentHeight)));

            // Snap the working icon size to the nearest entry the slider can represent.
            _working.IconSize = _iconSizes[IconSizeIndex(_working.IconSize)];

            Render(
                layoutPage,
                BeginGroup(layoutPage, "Icons Settings"),
                SliderRow(
                    "Icon Size",
                    "Size of an icon in both Preview and Micro Taskbar view modes. Icons are drawn as squares, so a single value is enough.\n\n16 uses the small window icon. Larger values use the window's large icon, so the shown logo matches the one in the Windows taskbar.",
                    0,
                    _iconSizes.Length - 1,
                    1,
                    1,
                    s => IconSizeIndex(s.IconSize),
                    (s, p) => s.IconSize = _iconSizes[p],
                    p => $"{_iconSizes[p]} px",
                    p => _iconSizes[p] != _settingsBefore.IconSize));

            Render(
                layoutPage,
                BeginGroup(layoutPage, "MicroTaskbar"),
                NumericRow(
                    "Icon Padding X",
                    "Negative values are supported as well so less space is taken and the icons look stacked.",
                    -9999,
                    9999,
                    s => s.IconPaddingX,
                    (s, v) => s.IconPaddingX = v),
                NumericRow(
                    "Icon Padding Y",
                    "Negative values are supported as well in case of a very small pager height, like 40px which is usually the size of the Windows Taskbar.",
                    -9999,
                    9999,
                    s => s.IconPaddingY,
                    (s, v) => s.IconPaddingY = v),
                ComboRow(
                    "Show When Minimized",
                    "Defines whether minimized windows are shown in the Micro Taskbar view mode.\n\nYes: Minimized windows are shown in their normal order.\n\nYes, after the others: Minimized windows are shown after all normal windows.\n\nNo: Minimized windows are hidden.\n\nMinimized windows are painted darker to tell them apart from normal windows.",
                    new[] { "Yes", "Yes, after the others", "No" },
                    s => MinimizedVisibilityIndex(s.ShowWhenMinimized),
                    (s, v) => s.ShowWhenMinimized = MinimizedVisibilityFromIndex(v)));

            return layoutPage;
        }

        private TabPage CreateThemeTab()
        {
            TabPage themePage = CreateTabPage("Theme");

            Render(
                themePage,
                BeginGroup(themePage, "General"),
                SliderRow(
                    "App Background Opacity",
                    null,
                    25,
                    100,
                    5,
                    10,
                    s => (int)Math.Round(s.BackgroundOpacity * 100),
                    (s, p) => s.BackgroundOpacity = p / 100.0,
                    p => FormatOpacity(p / 100.0),
                    p => Math.Abs((p / 100.0) - _settingsBefore.BackgroundOpacity) > 0.0001));

            Render(
                themePage,
                BeginGroup(themePage, "Desktops"),
                ComboRow(
                    "Border Style",
                    null,
                    new[] { "Box", "Underline" },
                    s => (int)s.DesktopBorderStyle,
                    (s, v) => s.DesktopBorderStyle = (Config.DesktopBorderStyle)v),
                ColorRow("Background Color", s => s.DesktopBackgroundColor,
                    (s, c) => s.DesktopBackgroundColor = c),
                ColorRow("Border Color", s => s.DesktopBorderColor,
                    (s, c) => s.DesktopBorderColor = c),
                ColorRow("Border Color (Active)", s => s.ActiveDesktopBorderColor,
                    (s, c) => s.ActiveDesktopBorderColor = c));

            Render(
                themePage,
                BeginGroup(themePage, "Preview Windows"),
                ColorRow("Background Color", s => s.WindowBackgroundColor,
                    (s, c) => s.WindowBackgroundColor = c),
                ColorRow("Border Color", s => s.WindowBorderColor,
                    (s, c) => s.WindowBorderColor = c),
                ColorRow("Border Color (Active)", s => s.ActiveWindowBorderColor,
                    (s, c) => s.ActiveWindowBorderColor = c),
                ColorRow("Background Color (Active)", s => s.ActiveWindowBackgroundColor,
                    (s, c) => s.ActiveWindowBackgroundColor = c));

            return themePage;
        }

        private TabPage CreateMiscTab()
        {
            TabPage miscPage = CreateTabPage("Misc");

            Render(
                miscPage,
                BeginGroup(miscPage, "Behaviour"),
                ComboRow(
                    "Desktop Change: Window Activation",
                    "Defines which window gets activated when another desktop is selected in the pager.\n\nDesktop's last window: The last activated window on the selected desktop is reactivated automatically.\n\nWindow under cursor: The app (icon or window) that is under the mouse pointer when the desktop is selected is activated instead.\n\nNone: No window is activated, so the currently active window stays unchanged.",
                    new[] { "Desktop's last window", "Window under cursor", "None" },
                    s => (int)s.DesktopChangeActivation,
                    (s, v) => s.DesktopChangeActivation = (DesktopChangeActivationMode)v));

            RowSpec primaryDelay = TextRow(
                "Primary Update Delay (ms)",
                "Delay for the main refresh cycle. Affects how often the currently opened windows (on all virtual desktops) are checked for changes.\n\nLower values update the pager faster, but can increase CPU usage.",
                s => s.PrimaryUpdateDelay,
                (s, v) => s.PrimaryUpdateDelay = v);

            RowSpec secondaryDelay = TextRow(
                "Secondary Update Delay (ms)",
                "Delay for background/secondary updates. Affects how often the app window is checked for being covered by another one, this includes the Windows Taskbar. Also (re-)pins the app to all virtual desktops.\n\nSome of those operations require more processing, that's why this delay should be higher than the primary one.",
                s => s.SecondaryUpdateDelay,
                (s, v) => s.SecondaryUpdateDelay = v);

            _primaryDelayBox = (TextBox)primaryDelay.Editor;
            _secondaryDelayBox = (TextBox)secondaryDelay.Editor;

            Render(miscPage, BeginGroup(miscPage, "Performance"), primaryDelay, secondaryDelay);

            TableLayoutPanel pinningGroup = BeginGroup(miscPage, "App Pinning");

            Render(
                miscPage,
                pinningGroup,
                CheckBoxRow(
                    "Show App In Taskbar",
                    null,
                    s => s.ShowAppInTaskbar,
                    (s, v) => s.ShowAppInTaskbar = v));

            var noteBox = new Panel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                BorderStyle = BorderStyle.FixedSingle,
                Padding = new Padding(8),
                Margin = new Padding(6, 12, 6, 4),
            };

            var note = new Label
            {
                Text = "Important: When deactivated, the automatic pinning of the app (to be visible on all virtual desktops) can't work anymore because of a limitation of the Windows OS. So the pinning has to be done manually after every application start using the \"Active Applications\" screen (Win + Tab).",
                AutoSize = true,
                MaximumSize = new Size(420, 0),
                ForeColor = SystemColors.GrayText,
                Margin = new Padding(8, 4, 8, 4),
            };
            noteBox.Controls.Add(note);

            AddSpanningControl(pinningGroup, noteBox);

            return miscPage;
        }

        // ------------------------------------------------------------------------------------
        // Commands
        // ------------------------------------------------------------------------------------

        private void Save_Click(object sender, EventArgs e)
        {
            AppSettings settings = TryBuildSettings(true);
            if (settings == null)
            {
                return;
            }

            Settings = settings;
            DialogResult = DialogResult.OK;
            Close();
        }

        private void Cancel_Click(object sender, EventArgs e)
        {
            DialogResult = DialogResult.Cancel;
            Close();
        }

        private void Defaults_Click(object sender, EventArgs e)
        {
            TabPage page = _tabs.SelectedTab;
            foreach (RowBinding row in _rows)
            {
                if (row.Page == page)
                {
                    Suppress(() => row.Load(_defaults));
                }
            }

            RefreshRevertButtons();
            RequestPreview();
        }

        // ------------------------------------------------------------------------------------
        // Editing state
        // ------------------------------------------------------------------------------------

        private void OnEditorChanged()
        {
            if (_initializing || _suppressEvents)
            {
                return;
            }

            RefreshRevertButtons();
            RequestPreview();
        }

        private void Suppress(Action action)
        {
            bool previous = _suppressEvents;
            _suppressEvents = true;
            try
            {
                action();
            }
            finally
            {
                _suppressEvents = previous;
            }
        }

        private void RefreshRevertButtons()
        {
            foreach (RowBinding row in _rows)
            {
                row.Revert.Visible = row.IsModified();
            }
        }

        private void RequestPreview()
        {
            if (_initializing || _suppressEvents)
            {
                return;
            }

            // Coalesce rapid changes (e.g. holding a NumericUpDown arrow) so the pager is not
            // rebuilt and retopmosted for every single increment.
            _previewTimer.Stop();
            _previewTimer.Start();
        }

        private void FlushPreview()
        {
            if (_closing || _initializing || _suppressEvents)
            {
                return;
            }

            AppSettings preview = TryBuildSettings(false);
            if (preview != null)
            {
                PreviewChanged?.Invoke(preview);
            }
        }

        private AppSettings TryBuildSettings(bool showValidationErrors)
        {
            if (!int.TryParse(
                    (string)_primaryDelayBox.Text,
                    NumberStyles.Integer,
                    CultureInfo.InvariantCulture,
                    out int primaryDelay)
                || primaryDelay < 1)
            {
                if (showValidationErrors)
                {
                    ShowValidationError("Primary Update Delay must be at least 1 ms.");
                }

                return null;
            }

            if (!int.TryParse(
                    (string)_secondaryDelayBox.Text,
                    NumberStyles.Integer,
                    CultureInfo.InvariantCulture,
                    out int secondaryDelay)
                || secondaryDelay < 100)
            {
                if (showValidationErrors)
                {
                    ShowValidationError("Secondary Update Delay must be at least 100 ms.");
                }

                return null;
            }

            AppSettings result = AppSettings.Clone(_working);
            result.PrimaryUpdateDelay = primaryDelay;
            result.SecondaryUpdateDelay = secondaryDelay;
            return result;
        }

        private void ShowValidationError(string message)
        {
            MessageBox.Show(this, message, "Invalid Settings", MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }

        private string FormatOpacity(double opacity)
        {
            return opacity.ToString("0.00", CultureInfo.InvariantCulture);
        }

        private Color PickColor(Color initial)
        {
            using (var dialog = new ColorDialog())
            {
                dialog.FullOpen = true;
                dialog.Color = initial;
                return dialog.ShowDialog() == DialogResult.OK ? dialog.Color : initial;
            }
        }

        // ------------------------------------------------------------------------------------
        // Row rendering
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// Renders the given rows into the group table in order, adding a label, editor and revert
        /// slot per row and registering each row against the page so revert and the per-tab Defaults
        /// action can reach it. All change, load and modified wiring lives here, so rows cannot drift
        /// apart.
        /// </summary>
        private void Render(TabPage page, TableLayoutPanel table, params RowSpec[] specs)
        {
            foreach (RowSpec spec in specs)
            {
                int row = AddTableRow(table);

                var label = new Label
                {
                    Text = spec.Label,
                    AutoSize = true,
                    Anchor = AnchorStyles.Left,
                    Margin = new Padding(6, 9, 6, 6),
                };

                if (!string.IsNullOrEmpty(spec.ToolTip))
                {
                    _toolTip.SetToolTip(label, spec.ToolTip);
                }

                spec.Editor.Anchor = AnchorStyles.Left;
                spec.Editor.Margin = new Padding(3, 5, 3, 5);

                var revert = new Button
                {
                    Text = "\u21BA",
                    Size = new Size(28, 24),
                    Location = new Point(0, 2),
                    Visible = spec.IsModified(),
                };
                _toolTip.SetToolTip(revert, "Revert to default");
                revert.Click += (s, e) =>
                {
                    Suppress(() => spec.Load(_settingsBefore));
                    RefreshRevertButtons();
                    RequestPreview();
                };

                // The host keeps the revert slot reserved even while the button is hidden,
                // otherwise WinForms would reflow the whole row on every visibility change.
                var revertHost = new Panel
                {
                    Size = new Size(32, 28),
                    Margin = new Padding(4, 4, 0, 4),
                    Anchor = AnchorStyles.Left,
                };
                revertHost.Controls.Add(revert);

                table.Controls.Add(label, 0, row);
                table.Controls.Add(spec.Editor, 1, row);
                table.Controls.Add(revertHost, 2, row);

                _rows.Add(new RowBinding(page, revert, spec.Load, spec.IsModified));
            }
        }

        /// <summary>
        /// Builds a row from a single int-valued editor such as a combo selection or numeric value.
        /// Label and toolTip are the row caption and optional hover text; editor is placed in the
        /// middle column. Read and show move the value between the editor and its stored form, get
        /// and set move it between a settings snapshot and the working copy, and subscribe registers
        /// the editor's change event so edits flow into the working settings.
        /// </summary>
        /// <returns>A row spec that also loads revert/default values and tracks modification state.</returns>
        private RowSpec LinkedRow(
            string label,
            string toolTip,
            Control editor,
            Func<int> read,
            Action<int> show,
            Action<Action> subscribe,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set)
        {
            subscribe(() =>
            {
                set(_working, read());
                OnEditorChanged();
            });

            return new RowSpec(
                label,
                toolTip,
                editor,
                s =>
                {
                    int value = get(s);
                    show(value);
                    set(_working, value);
                },
                () => read() != get(_settingsBefore));
        }

        /// <summary>
        /// Creates a drop-down row. Label and toolTip are the caption and optional hover text, items
        /// define both the entries and the stored index order, and get/set map that index to and from
        /// the settings.
        /// </summary>
        /// <returns>The row spec for the drop-down.</returns>
        private RowSpec ComboRow(
            string label,
            string toolTip,
            string[] items,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set)
        {
            var editor = new ComboBox
            {
                DropDownStyle = ComboBoxStyle.DropDownList,
                Width = 180,
            };
            editor.Items.AddRange(items);
            editor.SelectedIndex = get(_working);

            return LinkedRow(
                label,
                toolTip,
                editor,
                () => editor.SelectedIndex,
                index => editor.SelectedIndex = index,
                handler => editor.SelectedIndexChanged += (s, e) => handler(),
                get,
                set);
        }

        /// <summary>
        /// Creates a numeric row. Label and toolTip are the caption and optional hover text, min and
        /// max clamp the editor, and get/set map the value to and from the settings. An optional
        /// normalize callback adjusts a freshly entered value (for a dynamic lower bound) before it
        /// is written back.
        /// </summary>
        /// <returns>The row spec for the numeric editor.</returns>
        private RowSpec NumericRow(
            string label,
            string toolTip,
            int min,
            int max,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set,
            Func<int, int> normalize = null)
        {
            var editor = new NumericUpDown
            {
                Minimum = min,
                Maximum = max,
                Width = 150,
                TextAlign = HorizontalAlignment.Right,
                Value = Math.Max(min, Math.Min(max, get(_working))),
            };
            editor.ValueChanged += (s, e) =>
            {
                int value = (int)editor.Value;
                if (normalize != null)
                {
                    int normalized = Math.Max(min, Math.Min(max, normalize(value)));
                    if (normalized != value)
                    {
                        value = normalized;
                        Suppress(() => editor.Value = value);
                    }
                }

                set(_working, value);
                OnEditorChanged();
            };

            return new RowSpec(
                label,
                toolTip,
                editor,
                s =>
                {
                    editor.Value = Math.Max(min, Math.Min(max, get(s)));
                    set(_working, (int)editor.Value);
                },
                () => (int)editor.Value != get(_settingsBefore));
        }

        /// <summary>
        /// Creates a slider row with a companion value label. Min and max bound the slider position,
        /// tickFrequency and largeChange tune its stepping, and getPosition/setPosition translate
        /// between a settings value and that integer position. Label formats the position for the
        /// value label, and isModified decides whether a position differs from the initial settings.
        /// </summary>
        /// <returns>The row spec for the slider.</returns>
        private RowSpec SliderRow(
            string label,
            string toolTip,
            int min,
            int max,
            int tickFrequency,
            int largeChange,
            Func<AppSettings, int> getPosition,
            Action<AppSettings, int> setPosition,
            Func<int, string> format,
            Func<int, bool> isModified)
        {
            var trackBar = new TrackBar
            {
                Minimum = min,
                Maximum = max,
                TickFrequency = tickFrequency,
                LargeChange = largeChange,
                SmallChange = 1,
                Value = Math.Max(min, Math.Min(max, getPosition(_working))),
                AutoSize = false,
                Width = 140,
                Height = 32,
                Margin = new Padding(0, 6, 4, 0),
            };

            var valueLabel = new Label
            {
                AutoSize = true,
                Text = format(trackBar.Value),
                Margin = new Padding(4, 14, 0, 0),
            };

            var host = new FlowLayoutPanel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                WrapContents = false,
                Margin = new Padding(0),
            };
            host.Controls.Add(trackBar);
            host.Controls.Add(valueLabel);

            trackBar.ValueChanged += (s, e) =>
            {
                setPosition(_working, trackBar.Value);
                valueLabel.Text = format(trackBar.Value);
                OnEditorChanged();
            };

            return new RowSpec(
                label,
                toolTip,
                host,
                s =>
                {
                    int position = Math.Max(min, Math.Min(max, getPosition(s)));
                    trackBar.Value = position;
                    valueLabel.Text = format(position);
                    setPosition(_working, position);
                },
                () => isModified(trackBar.Value));
        }

        /// <summary>
        /// Creates a boolean row. Label and toolTip are the caption and optional hover text, and
        /// get/set read and write the flag in the settings.
        /// </summary>
        /// <returns>The row spec for the check box.</returns>
        private RowSpec CheckBoxRow(
            string label,
            string toolTip,
            Func<AppSettings, bool> get,
            Action<AppSettings, bool> set)
        {
            var editor = new CheckBox
            {
                AutoSize = true,
                Checked = get(_working),
            };
            editor.CheckedChanged += (s, e) =>
            {
                set(_working, editor.Checked);
                OnEditorChanged();
            };

            return new RowSpec(
                label,
                toolTip,
                editor,
                s =>
                {
                    editor.Checked = get(s);
                    set(_working, editor.Checked);
                },
                () => editor.Checked != get(_settingsBefore));
        }

        /// <summary>
        /// Creates an integer text row. Label and toolTip are the caption and optional hover text,
        /// and get/set read and write the value. Unparsable text leaves the working setting unchanged
        /// and marks the row as modified so Save reports a validation error.
        /// </summary>
        /// <returns>The row spec for the text box.</returns>
        private RowSpec TextRow(
            string label,
            string toolTip,
            Func<AppSettings, int> get,
            Action<AppSettings, int> set)
        {
            var editor = new TextBox
            {
                Width = 150,
                TextAlign = HorizontalAlignment.Right,
            };
            editor.Text = get(_working).ToString(CultureInfo.InvariantCulture);
            editor.TextChanged += (s, e) =>
            {
                if (int.TryParse(editor.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out int value))
                {
                    set(_working, value);
                }

                OnEditorChanged();
            };

            return new RowSpec(
                label,
                toolTip,
                editor,
                s => editor.Text = get(s).ToString(CultureInfo.InvariantCulture),
                () => !int.TryParse(editor.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out int value)
                    || value != get(_settingsBefore));
        }

        /// <summary>
        /// Creates a color row with a swatch and a picker button. Label is the caption, and get/set
        /// read and write the color in the settings.
        /// </summary>
        /// <returns>The row spec for the color swatch and picker.</returns>
        private RowSpec ColorRow(
            string label,
            Func<AppSettings, Color> get,
            Action<AppSettings, Color> set)
        {
            var preview = new Panel
            {
                Size = new Size(32, 24),
                BorderStyle = BorderStyle.FixedSingle,
                BackColor = get(_working),
                Margin = new Padding(0, 4, 6, 0),
            };

            var choose = new Button
            {
                Text = "Choose...",
                Size = new Size(80, 24),
                Margin = new Padding(0, 4, 0, 0),
            };
            choose.Click += (s, e) =>
            {
                Color picked = PickColor(get(_working));
                if (picked.ToArgb() == get(_working).ToArgb())
                {
                    return;
                }

                set(_working, picked);
                preview.BackColor = picked;
                OnEditorChanged();
            };

            var host = new FlowLayoutPanel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                WrapContents = false,
                Margin = new Padding(0),
            };
            host.Controls.Add(preview);
            host.Controls.Add(choose);

            return new RowSpec(
                label,
                null,
                host,
                s =>
                {
                    set(_working, get(s));
                    preview.BackColor = get(s);
                },
                () => get(_working).ToArgb() != get(_settingsBefore).ToArgb());
        }

        // ------------------------------------------------------------------------------------
        // Layout helpers
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// Appends a row to the given table whose control spans all three columns, e.g. the pinning
        /// note box.
        /// </summary>
        private void AddSpanningControl(TableLayoutPanel table, Control control)
        {
            int row = AddTableRow(table);
            control.Dock = DockStyle.Fill;
            table.Controls.Add(control, 0, row);
            table.SetColumnSpan(control, 3);
        }

        /// <summary>
        /// Creates the empty shell of a settings tab titled with the given caption: a scrolling page
        /// with a one-column stack. The stack is kept in the page tag for BeginGroup.
        /// </summary>
        /// <returns>The prepared tab page.</returns>
        private TabPage CreateTabPage(string title)
        {
            var page = new TabPage(title);
            var scroll = new Panel
            {
                Dock = DockStyle.Fill,
                AutoScroll = true,
            };

            var stack = new TableLayoutPanel
            {
                Dock = DockStyle.Top,
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                ColumnCount = 1,
                Padding = new Padding(2),
            };
            stack.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));

            scroll.Controls.Add(stack);
            page.Controls.Add(scroll);
            page.Tag = stack;
            return page;
        }

        /// <summary>
        /// Adds a group box with the given caption to the page's stack and returns the three-column
        /// table inside it, which is the target for Render.
        /// </summary>
        /// <returns>The table that receives the group's rows.</returns>
        private TableLayoutPanel BeginGroup(TabPage page, string title)
        {
            var stack = (TableLayoutPanel)page.Tag;
            var group = new GroupBox
            {
                Text = title,
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                Dock = DockStyle.Top,
                Margin = new Padding(6, 4, 6, 4),
            };

            var table = new TableLayoutPanel
            {
                AutoSize = true,
                AutoSizeMode = AutoSizeMode.GrowAndShrink,
                Dock = DockStyle.Top,
                ColumnCount = 3,
                Padding = new Padding(10, 10, 10, 10),
            };
            table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 200));
            table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 190));
            table.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 36));

            group.Controls.Add(table);

            int row = AddTableRow(stack);
            stack.Controls.Add(group, 0, row);

            return table;
        }

        // ------------------------------------------------------------------------------------
        // Placement helpers
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// Chooses a dialog location relative to the pager: right, left, below, above, then centered
        /// as a last resort. The owner bounds and dialog size are both required, and every candidate
        /// must fit fully inside one screen's working area.
        /// </summary>
        /// <returns>The preferred top-left position.</returns>
        private Point FindPlacement(Rectangle owner, Size size)
        {
            return TryPlaceRight(owner, size)
                ?? TryPlaceLeft(owner, size)
                ?? TryPlaceBelow(owner, size)
                ?? TryPlaceAbove(owner, size)
                ?? CenterOverOwner(owner, size);
        }

        private Point? TryPlaceRight(Rectangle owner, Size size)
        {
            int x = owner.Right + PlacementGap;
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle area = screen.WorkingArea;
                if (x >= area.Left && x + size.Width <= area.Right && size.Height <= area.Height)
                {
                    int y = Math.Max(area.Top, Math.Min(owner.Top, area.Bottom - size.Height));
                    return new Point(x, y);
                }
            }

            return null;
        }

        private Point? TryPlaceLeft(Rectangle owner, Size size)
        {
            int x = owner.Left - PlacementGap - size.Width;
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle area = screen.WorkingArea;
                if (x >= area.Left && x + size.Width <= area.Right && size.Height <= area.Height)
                {
                    int y = Math.Max(area.Top, Math.Min(owner.Top, area.Bottom - size.Height));
                    return new Point(x, y);
                }
            }

            return null;
        }

        private Point? TryPlaceBelow(Rectangle owner, Size size)
        {
            int y = owner.Bottom + PlacementGap;
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle area = screen.WorkingArea;
                if (y >= area.Top && y + size.Height <= area.Bottom && size.Width <= area.Width)
                {
                    int x = Math.Max(area.Left, Math.Min(owner.Left, area.Right - size.Width));
                    return new Point(x, y);
                }
            }

            return null;
        }

        private Point? TryPlaceAbove(Rectangle owner, Size size)
        {
            int y = owner.Top - PlacementGap - size.Height;
            foreach (Screen screen in Screen.AllScreens)
            {
                Rectangle area = screen.WorkingArea;
                if (y >= area.Top && y + size.Height <= area.Bottom && size.Width <= area.Width)
                {
                    int x = Math.Max(area.Left, Math.Min(owner.Left, area.Right - size.Width));
                    return new Point(x, y);
                }
            }

            return null;
        }

        private Point CenterOverOwner(Rectangle owner, Size size)
        {
            return new Point(
                owner.Left + ((owner.Width - size.Width) / 2),
                owner.Top + ((owner.Height - size.Height) / 2));
        }

        // ------------------------------------------------------------------------------------
        // Row model
        // ------------------------------------------------------------------------------------

        /// <summary>
        /// A single settings row in declarative form: the label, editor, reset behavior and
        /// modification check that Render turns into controls. Created by the spec factories.
        /// </summary>
        private sealed class RowSpec
        {
            /// <summary>
            /// Initializes a row description. Label is the caption, toolTip the optional hover text,
            /// and editor the middle control. Load applies a settings snapshot to the editor and
            /// working state, while isModified reports whether the row differs from the initial settings.
            /// </summary>
            public RowSpec(
                string label,
                string toolTip,
                Control editor,
                Action<AppSettings> load,
                Func<bool> isModified)
            {
                Label = label;
                ToolTip = toolTip;
                Editor = editor;
                Load = load;
                IsModified = isModified;
            }

            public string Label { get; }

            public string ToolTip { get; }

            public Control Editor { get; }

            public Action<AppSettings> Load { get; }

            public Func<bool> IsModified { get; }
        }

        /// <summary>
        /// A rendered row, remembered so the revert buttons and the per-tab Defaults action can
        /// reach it after construction.
        /// </summary>
        private sealed class RowBinding
        {
            /// <summary>
            /// Initializes a rendered-row binding. Page is the tab the row belongs to, revert is its
            /// button, load applies a settings snapshot, and isModified reports whether the row
            /// differs from the initial settings.
            /// </summary>
            public RowBinding(TabPage page, Button revert, Action<AppSettings> load, Func<bool> isModified)
            {
                Page = page;
                Revert = revert;
                Load = load;
                IsModified = isModified;
            }

            public TabPage Page { get; }

            public Button Revert { get; }

            public Action<AppSettings> Load { get; }

            public Func<bool> IsModified { get; }
        }
    }
}
