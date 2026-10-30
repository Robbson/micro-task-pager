using System;
using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using MicroTaskPager.Config;
using DrawingColor = System.Drawing.Color;
using DrawingPoint = System.Drawing.Point;
using DrawingRectangle = System.Drawing.Rectangle;
using DrawingSize = System.Drawing.Size;

namespace MicroTaskPager.GUI
{
    public partial class AppSettingsWindow : Form
    {
        private const int PlacementGap = 8;

        private readonly AppSettings _defaults = new AppSettings();
        private readonly AppSettings _settingsBefore;
        private AppSettings _previewSettings;

        private DrawingColor _desktopBackgroundColor;
        private DrawingColor _desktopBorderColor;
        private DrawingColor _activeDesktopBorderColor;
        private DrawingColor _windowBackgroundColor;
        private DrawingColor _activeWindowBackgroundColor;
        private DrawingColor _windowBorderColor;
        private DrawingColor _activeWindowBorderColor;

        private Rectangle _placementOwnerBounds;
        private bool _hasPlacementOwner = false;

        // Tab page & scroll panels
        private TabControl tabControl;
        private TabPage tabPageLayout;
        private TabPage tabPageTheme;
        private TabPage tabPageMisc;
        private Panel scrollLayout;
        private Panel scrollTheme;
        private Panel scrollMisc;

        // Layout tab controls
        private ComboBox cmbViewModeDefault;
        private NumericUpDown numericPagerHeight;
        private NumericUpDown numericIconPaddingX;
        private NumericUpDown numericIconPaddingY;

        // Theme tab controls
        private TrackBar sldBackgroundOpacity;
        private Label txtBackgroundOpacityValue;
        private ComboBox cmbDesktopBorderStyle;
        private Panel previewBackgroundColor;
        private Panel previewDesktopColor;
        private Panel previewActiveDesktopBorderColor;
        private Panel previewWindowColor;
        private Panel previewActiveWindowColor;
        private Panel previewWindowBorderColor;
        private Panel previewActiveWindowBorderColor;

        // Revert buttons
        private Button btnRevertViewModeDefault;
        private Button btnRevertPagerHeight;
        private Button btnRevertIconPaddingX;
        private Button btnRevertIconPaddingY;
        private Button btnRevertBackgroundOpacity;
        private Button btnRevertDesktopBorderStyle;
        private Button btnRevertBackgroundColor;
        private Button btnRevertDesktopColor;
        private Button btnRevertActiveDesktopBorderColor;
        private Button btnRevertWindowColor;
        private Button btnRevertWindowBorderColor;
        private Button btnRevertActiveWindowBorderColor;
        private Button btnRevertActiveWindowColor;

        // Misc tab controls
        private TextBox txtPrimaryDelay;
        private TextBox txtSecondaryDelay;
        private CheckBox chkShowAppInTaskbar;
        private Button btnRevertPrimaryDelay;
        private Button btnRevertSecondaryDelay;
        private Button btnRevertShowAppInTaskbar;
        private FlowLayoutPanel buttonFlowLayoutPanel;
        private Button btnSave;
        private Button btnCancel;

        public AppSettingsWindow(AppSettings settings, Rectangle ownerBounds)
        {
            InitializeComponent();

            _settingsBefore = AppSettings.Clone(settings);
            _previewSettings = AppSettings.Clone(settings);

            // Populate view mode combo
            cmbViewModeDefault.Items.Add("Preview");
            cmbViewModeDefault.Items.Add("MicroTaskbar");
            cmbViewModeDefault.SelectedIndex = (int)settings.ViewModeDefault;

            numericPagerHeight.Value = settings.PagerHeight;
            numericIconPaddingX.Value = settings.IconPaddingX;
            numericIconPaddingY.Value = settings.IconPaddingY;

            sldBackgroundOpacity.Value = (int)(Math.Max(0.25, Math.Min(1.0, settings.BackgroundOpacity)) * 100);
            txtBackgroundOpacityValue.Text = FormatOpacity(sldBackgroundOpacity.Value / 100.0);

            cmbDesktopBorderStyle.Items.Add("Box");
            cmbDesktopBorderStyle.Items.Add("Underline");
            cmbDesktopBorderStyle.SelectedIndex = (int)settings.DesktopBorderStyle;

            _desktopBackgroundColor = settings.DesktopBackgroundColor;
            _desktopBorderColor = settings.DesktopBorderColor;
            _activeDesktopBorderColor = settings.ActiveDesktopBorderColor;
            _windowBackgroundColor = settings.WindowBackgroundColor;
            _activeWindowBackgroundColor = settings.ActiveWindowBackgroundColor;
            _windowBorderColor = settings.WindowBorderColor;
            _activeWindowBorderColor = settings.ActiveWindowBorderColor;

            txtPrimaryDelay.Text = settings.PrimaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
            txtSecondaryDelay.Text = settings.SecondaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
            chkShowAppInTaskbar.Checked = settings.ShowAppInTaskbar;

            _placementOwnerBounds = ownerBounds;
            _hasPlacementOwner = true;

            RefreshColorPreviews();
            UpdateRevertButtons();
        }

        public event Action<AppSettings> PreviewChanged;

        public AppSettings Settings => _previewSettings;

        private void RefreshColorPreviews()
        {
            if (previewBackgroundColor != null) previewBackgroundColor.BackColor = _desktopBackgroundColor;
            if (previewDesktopColor != null) previewDesktopColor.BackColor = _desktopBorderColor;
            if (previewActiveDesktopBorderColor != null) previewActiveDesktopBorderColor.BackColor = _activeDesktopBorderColor;
            if (previewWindowColor != null) previewWindowColor.BackColor = _windowBackgroundColor;
            if (previewActiveWindowColor != null) previewActiveWindowColor.BackColor = _activeWindowBackgroundColor;
            if (previewWindowBorderColor != null) previewWindowBorderColor.BackColor = _windowBorderColor;
            if (previewActiveWindowBorderColor != null) previewActiveWindowBorderColor.BackColor = _activeWindowBorderColor;
        }

        private void UpdateRevertButtons()
        {
            if (!_hasPlacementOwner) return;

            if (tabControl.SelectedTab == tabPageLayout)
            {
                UpdateRevertVisibility(btnRevertViewModeDefault, cmbViewModeDefault.SelectedIndex != (int)_settingsBefore.ViewModeDefault);
                UpdateRevertVisibility(btnRevertPagerHeight, numericPagerHeight.Value != _settingsBefore.PagerHeight);
                UpdateRevertVisibility(btnRevertIconPaddingX, numericIconPaddingX.Value != _settingsBefore.IconPaddingX);
                UpdateRevertVisibility(btnRevertIconPaddingY, numericIconPaddingY.Value != _settingsBefore.IconPaddingY);
            }
            else if (tabControl.SelectedTab == tabPageTheme)
            {
                double currentOpacity = sldBackgroundOpacity.Value / 100.0;
                UpdateRevertVisibility(btnRevertBackgroundOpacity, Math.Abs(currentOpacity - _settingsBefore.BackgroundOpacity) > 0.0001);
                UpdateRevertVisibility(btnRevertDesktopBorderStyle, cmbDesktopBorderStyle.SelectedIndex != (int)_settingsBefore.DesktopBorderStyle);
                UpdateRevertVisibility(btnRevertBackgroundColor, _desktopBackgroundColor.ToArgb() != _settingsBefore.DesktopBackgroundColor.ToArgb());
                UpdateRevertVisibility(btnRevertDesktopColor, _desktopBorderColor.ToArgb() != _settingsBefore.DesktopBorderColor.ToArgb());
                UpdateRevertVisibility(btnRevertActiveDesktopBorderColor, _activeDesktopBorderColor.ToArgb() != _settingsBefore.ActiveDesktopBorderColor.ToArgb());
                UpdateRevertVisibility(btnRevertWindowColor, _windowBackgroundColor.ToArgb() != _settingsBefore.WindowBackgroundColor.ToArgb());
                int activeWinArgb = (_activeWindowBackgroundColor.A << 24) | (_activeWindowBackgroundColor.R << 16) | (_activeWindowBackgroundColor.G << 8) | _activeWindowBackgroundColor.B;
                int beforeWinArgb = (_settingsBefore.ActiveWindowBackgroundColor.A << 24) | (_settingsBefore.ActiveWindowBackgroundColor.R << 16) | (_settingsBefore.ActiveWindowBackgroundColor.G << 8) | _settingsBefore.ActiveWindowBackgroundColor.B;
                UpdateRevertVisibility(btnRevertActiveWindowColor, activeWinArgb != beforeWinArgb);
                UpdateRevertVisibility(btnRevertWindowBorderColor, _windowBorderColor.ToArgb() != _settingsBefore.WindowBorderColor.ToArgb());
                UpdateRevertVisibility(btnRevertActiveWindowBorderColor, _activeWindowBorderColor.ToArgb() != _settingsBefore.ActiveWindowBorderColor.ToArgb());
            }
            else if (tabControl.SelectedTab == tabPageMisc)
            {
                bool primaryDifferent = !int.TryParse(txtPrimaryDelay.Text, out int d) || d != _settingsBefore.PrimaryUpdateDelay;
                UpdateRevertVisibility(btnRevertPrimaryDelay, primaryDifferent);
                bool secondaryDifferent = !int.TryParse(txtSecondaryDelay.Text, out int d2) || d2 != _settingsBefore.SecondaryUpdateDelay;
                UpdateRevertVisibility(btnRevertSecondaryDelay, secondaryDifferent);
                UpdateRevertVisibility(btnRevertShowAppInTaskbar, chkShowAppInTaskbar.Checked != _settingsBefore.ShowAppInTaskbar);
            }
        }

        private static void UpdateRevertVisibility(Button button, bool isVisible)
        {
            button.Visible = isVisible;
        }

        private static DrawingColor PickColor(DrawingColor initial)
        {
            using (var dlg = new ColorDialog())
            {
                dlg.FullOpen = true;
                dlg.Color = initial;
                return dlg.ShowDialog() == DialogResult.OK ? dlg.Color : initial;
            }
        }

        private void UpdateColorsTab()
        {
            RefreshColorPreviews();
            UpdateRevertButtons();
            RequestPreview();
        }

        private void RequestPreview()
        {
            var s = BuildSettingsFromControls();
            if (s != null)
            {
                PreviewChanged?.Invoke(s);
            }
        }

        private AppSettings BuildSettingsFromControls()
        {
            if (!TryParsePositiveInt(txtPrimaryDelay.Text, 1, out int primaryDelay))
            {
                ShowValidationError("Primary Update Delay must be at least 1 ms.");
                return null;
            }

            if (!TryParsePositiveInt(txtSecondaryDelay.Text, 100, out int secondaryDelay))
            {
                ShowValidationError("Secondary Update Delay must be at least 100 ms.");
                return null;
            }

            double opacity = sldBackgroundOpacity.Value / 100.0;
            opacity = Math.Max(0.25, Math.Min(1.0, opacity));

            return new AppSettings
            {
                ViewModeDefault = (ViewMode)(cmbViewModeDefault.SelectedIndex <= 0 ? 0 : 1),
                RendererViewModes = _previewSettings.RendererViewModes,
                PagerHeight = (int)numericPagerHeight.Value,
                IconPaddingX = (int)numericIconPaddingX.Value,
                IconPaddingY = (int)numericIconPaddingY.Value,
                BackgroundOpacity = opacity,
                DesktopBorderStyle = (Config.DesktopBorderStyle)(cmbDesktopBorderStyle.SelectedIndex <= 0 ? 0 : 1),
                DesktopBackgroundColor = _desktopBackgroundColor,
                DesktopBorderColor = _desktopBorderColor,
                ActiveDesktopBorderColor = _activeDesktopBorderColor,
                WindowBackgroundColor = _windowBackgroundColor,
                ActiveWindowBackgroundColor = _activeWindowBackgroundColor,
                WindowBorderColor = _windowBorderColor,
                ActiveWindowBorderColor = _activeWindowBorderColor,
                PrimaryUpdateDelay = primaryDelay,
                SecondaryUpdateDelay = secondaryDelay,
                ShowAppInTaskbar = chkShowAppInTaskbar.Checked,
            };
        }

        private bool TryParsePositiveInt(string input, int min, out int value)
        {
            return int.TryParse(input, NumberStyles.Integer, CultureInfo.InvariantCulture, out value) && value >= min;
        }

        private void ShowValidationError(string message)
        {
            MessageBox.Show(this, message, "Invalid Settings", MessageBoxButtons.OK, MessageBoxIcon.Warning);
        }

        private string FormatOpacity(double opacity)
        {
            return opacity.ToString("0.00", CultureInfo.InvariantCulture);
        }

        // ===================== Event Handlers =====================

        private void ResetToDefaults_Click(object sender, EventArgs e)
        {
            if (tabControl.SelectedTab == tabPageLayout)
            {
                cmbViewModeDefault.SelectedIndex = _defaults.ViewModeDefault <= 0 ? 0 : 1;
                numericPagerHeight.Value = _defaults.PagerHeight;
                numericIconPaddingX.Value = _defaults.IconPaddingX;
                numericIconPaddingY.Value = _defaults.IconPaddingY;
            }
            else if (tabControl.SelectedTab == tabPageTheme)
            {
                sldBackgroundOpacity.Value = (int)(Math.Max(0.25, Math.Min(1.0, _defaults.BackgroundOpacity)) * 100);
                txtBackgroundOpacityValue.Text = FormatOpacity(sldBackgroundOpacity.Value / 100.0);
                cmbDesktopBorderStyle.SelectedIndex = _defaults.DesktopBorderStyle <= 0 ? 0 : 1;
                _desktopBackgroundColor = _defaults.DesktopBackgroundColor;
                _desktopBorderColor = _defaults.DesktopBorderColor;
                _activeDesktopBorderColor = _defaults.ActiveDesktopBorderColor;
                _windowBackgroundColor = _defaults.WindowBackgroundColor;
                _activeWindowBackgroundColor = _defaults.ActiveWindowBackgroundColor;
                _windowBorderColor = _defaults.WindowBorderColor;
                _activeWindowBorderColor = _defaults.ActiveWindowBorderColor;
                RefreshColorPreviews();
            }
            else if (tabControl.SelectedTab == tabPageMisc)
            {
                txtPrimaryDelay.Text = _defaults.PrimaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
                txtSecondaryDelay.Text = _defaults.SecondaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
                chkShowAppInTaskbar.Checked = _defaults.ShowAppInTaskbar;
            }

            RefreshColorPreviews();
            UpdateRevertButtons();
        }

        private void TabControl_SelectedIndexChanged(object sender, EventArgs e)
        {
            UpdateRevertButtons();
        }

        private void ViewModeDefaultChanged(object sender, EventArgs e)
        {
            UpdateRevertButtons();
            RequestPreview();
        }

        private void RevertViewModeDefault_Click(object sender, EventArgs e)
        {
            cmbViewModeDefault.SelectedIndex = _settingsBefore.ViewModeDefault <= 0 ? 0 : 1;
            UpdateRevertButtons();
        }

        private void PagerHeightChanged(object sender, EventArgs e)
        {
            var height = numericPagerHeight.Value - numericIconPaddingY.Value;
            if (height < 16)
            {
                numericPagerHeight.Value += 16 - height;
            }
            UpdateRevertButtons();
            RequestPreview();
        }

        private void RevertPagerHeight_Click(object sender, EventArgs e)
        {
            numericPagerHeight.Value = _settingsBefore.PagerHeight;
            UpdateRevertButtons();
            RequestPreview();
        }

        private void IconPaddingXChanged(object sender, EventArgs e)
        {
            UpdateRevertButtons();
            RequestPreview();
        }

        private void IconPaddingYChanged(object sender, EventArgs e)
        {
            UpdateRevertButtons();
            RequestPreview();
        }

        private void RevertIconPaddingX_Click(object sender, EventArgs e)
        {
            numericIconPaddingX.Value = _settingsBefore.IconPaddingX;
            UpdateRevertButtons();
        }

        private void RevertIconPaddingY_Click(object sender, EventArgs e)
        {
            numericIconPaddingY.Value = _settingsBefore.IconPaddingY;
            UpdateRevertButtons();
        }

        private void BackgroundOpacityChanged(object sender, EventArgs e)
        {
            txtBackgroundOpacityValue.Text = FormatOpacity(sldBackgroundOpacity.Value / 100.0);
            UpdateRevertButtons();
            RequestPreview();
        }

        private void RevertBackgroundOpacity_Click(object sender, EventArgs e)
        {
            sldBackgroundOpacity.Value = (int)(Math.Max(0.25, Math.Min(1.0, _settingsBefore.BackgroundOpacity)) * 100);
            txtBackgroundOpacityValue.Text = FormatOpacity(sldBackgroundOpacity.Value / 100.0);
            UpdateRevertButtons();
            RequestPreview();
        }

        private void DesktopBorderStyleChanged(object sender, EventArgs e)
        {
            UpdateRevertButtons();
            RequestPreview();
        }

        private void RevertDesktopBorderStyle_Click(object sender, EventArgs e)
        {
            cmbDesktopBorderStyle.SelectedIndex = _settingsBefore.DesktopBorderStyle <= 0 ? 0 : 1;
            UpdateRevertButtons();
            RequestPreview();
        }

        private void ChooseDesktopBackgroundColor_Click(object sender, EventArgs e)
        {
            _desktopBackgroundColor = PickColor(_desktopBackgroundColor);
            UpdateColorsTab();
        }

        private void RevertDesktopBackgroundColor_Click(object sender, EventArgs e)
        {
            _desktopBackgroundColor = _settingsBefore.DesktopBackgroundColor;
            UpdateColorsTab();
        }

        private void ChooseDesktopBorderColor_Click(object sender, EventArgs e)
        {
            _desktopBorderColor = PickColor(_desktopBorderColor);
            UpdateColorsTab();
        }

        private void RevertDesktopBorderColor_Click(object sender, EventArgs e)
        {
            _desktopBorderColor = _settingsBefore.DesktopBorderColor;
            UpdateColorsTab();
        }

        private void ChooseActiveDesktopBorderColor_Click(object sender, EventArgs e)
        {
            _activeDesktopBorderColor = PickColor(_activeDesktopBorderColor);
            UpdateColorsTab();
        }

        private void RevertActiveDesktopBorderColor_Click(object sender, EventArgs e)
        {
            _activeDesktopBorderColor = _settingsBefore.ActiveDesktopBorderColor;
            UpdateColorsTab();
        }

        private void ChooseWindowBackgroundColor_Click(object sender, EventArgs e)
        {
            _windowBackgroundColor = PickColor(_windowBackgroundColor);
            UpdateColorsTab();
        }

        private void RevertWindowBackgroundColor_Click(object sender, EventArgs e)
        {
            _windowBackgroundColor = _settingsBefore.WindowBackgroundColor;
            UpdateColorsTab();
        }

        private void ChooseActiveWindowBackgroundColor_Click(object sender, EventArgs e)
        {
            _activeWindowBackgroundColor = PickColor(_activeWindowBackgroundColor);
            UpdateColorsTab();
        }

        private void RevertActiveWindowBackgroundColor_Click(object sender, EventArgs e)
        {
            _activeWindowBackgroundColor = _settingsBefore.ActiveWindowBackgroundColor;
            UpdateColorsTab();
        }

        private void ChooseWindowBorderColor_Click(object sender, EventArgs e)
        {
            _windowBorderColor = PickColor(_windowBorderColor);
            UpdateColorsTab();
        }

        private void RevertWindowBorderColor_Click(object sender, EventArgs e)
        {
            _windowBorderColor = _settingsBefore.WindowBorderColor;
            UpdateColorsTab();
        }

        private void ChooseActiveWindowBorderColor_Click(object sender, EventArgs e)
        {
            _activeWindowBorderColor = PickColor(_activeWindowBorderColor);
            UpdateColorsTab();
        }

        private void RevertActiveWindowBorderColor_Click(object sender, EventArgs e)
        {
            _activeWindowBorderColor = _settingsBefore.ActiveWindowBorderColor;
            UpdateColorsTab();
        }

        private void MiscTextInputChanged(object sender, EventArgs e)
        {
            UpdateRevertButtons();
        }

        private void MiscBooleanInputChanged(object sender, EventArgs e)
        {
            UpdateRevertButtons();
        }

        private void RevertPrimaryDelay_Click(object sender, EventArgs e)
        {
            txtPrimaryDelay.Text = _settingsBefore.PrimaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
            UpdateRevertButtons();
        }

        private void RevertSecondaryDelay_Click(object sender, EventArgs e)
        {
            txtSecondaryDelay.Text = _settingsBefore.SecondaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
            UpdateRevertButtons();
        }

        private void RevertShowAppInTaskbar_Click(object sender, EventArgs e)
        {
            chkShowAppInTaskbar.Checked = _settingsBefore.ShowAppInTaskbar;
            UpdateRevertButtons();
        }

        private void btnSave_Click(object sender, EventArgs e)
        {
            var settings = BuildSettingsFromControls();
            if (settings == null) return;
            _previewSettings = settings;
            DialogResult = DialogResult.OK;
            Close();
        }

        private void btnCancel_Click(object sender, EventArgs e)
        {
            DialogResult = DialogResult.Cancel;
            Close();
        }

        private void InitializeComponent()
        {
            SuspendLayout();

            // --- Form setup ---
            Text = "Micro Task Pager Settings";
            Size = new System.Drawing.Size(460, 625);
            StartPosition = FormStartPosition.Manual;
            FormBorderStyle = FormBorderStyle.FixedDialog;
            MaximizeBox = false;
            MinimizeBox = false;
            ShowInTaskbar = false;
            AcceptButton = btnSave;
            CancelButton = btnCancel;

            // --- TabControl ---
            tabControl = new TabControl();
            tabPageLayout = new TabPage("Layout");
            tabPageTheme = new TabPage("Theme");
            tabPageMisc = new TabPage("Misc");

            scrollLayout = new Panel();
            scrollLayout.Dock = DockStyle.Fill;
            scrollLayout.AutoScroll = true;
            tabPageLayout.Controls.Add(scrollLayout);

            scrollTheme = new Panel();
            scrollTheme.Dock = DockStyle.Fill;
            scrollTheme.AutoScroll = true;
            tabPageTheme.Controls.Add(scrollTheme);

            scrollMisc = new Panel();
            scrollMisc.Dock = DockStyle.Fill;
            scrollMisc.AutoScroll = true;
            tabPageMisc.Controls.Add(scrollMisc);

            tabControl.Controls.Add(tabPageLayout);
            tabControl.Controls.Add(tabPageTheme);
            tabControl.Controls.Add(tabPageMisc);
            tabControl.Location = new Point(12, 12);
            tabControl.Size = new System.Drawing.Size(436, 540);
            tabControl.TabIndex = 0;
            tabControl.SelectedIndexChanged += TabControl_SelectedIndexChanged;

            // --- Buttons (Defaults, Save, Cancel) ---
            btnSave = new Button();
            btnSave.Text = "Save";
            btnSave.Size = new System.Drawing.Size(100, 25);
            btnSave.Margin = new Padding(0, 10, 8, 0);
            btnSave.Dock = DockStyle.Right;
            btnSave.Click += btnSave_Click;

            btnCancel = new Button();
            btnCancel.Text = "Cancel";
            btnCancel.Size = new System.Drawing.Size(100, 25);
            btnCancel.Margin = new Padding(0, 10, 0, 0);
            btnCancel.Dock = DockStyle.Right;
            btnCancel.Click += btnCancel_Click;

            var btnDefaults = new Button();
            btnDefaults.Text = "Defaults";
            btnDefaults.Size = new System.Drawing.Size(100, 25);
            btnDefaults.Margin = new Padding(0, 10, 8, 0);
            btnDefaults.Dock = DockStyle.Right;
            btnDefaults.Click += (s, e) => ResetToDefaults_Click(s, e);

            buttonFlowLayoutPanel = new FlowLayoutPanel();
            buttonFlowLayoutPanel.Location = new Point(12, 555);
            buttonFlowLayoutPanel.Size = new System.Drawing.Size(436, 30);
            buttonFlowLayoutPanel.AutoSize = false;
            buttonFlowLayoutPanel.AutoSizeMode = AutoSizeMode.GrowAndShrink;
            buttonFlowLayoutPanel.Controls.Add(btnDefaults);
            buttonFlowLayoutPanel.Controls.Add(btnSave);
            buttonFlowLayoutPanel.Controls.Add(btnCancel);

            // ============================ LAYOUT TAB ============================
            // GroupBox: Rendering
            {
                var grpRendering = new GroupBox();
                grpRendering.Text = "Rendering";
                grpRendering.Location = new Point(8, 8);
                grpRendering.Size = new System.Drawing.Size(420, 100);

                var lblViewModeDefault = new Label();
                lblViewModeDefault.Text = "View Mode Default";
                lblViewModeDefault.AutoSize = true;
                lblViewModeDefault.Location = new Point(15, 20);

                cmbViewModeDefault = new ComboBox();
                cmbViewModeDefault.DropDownStyle = ComboBoxStyle.DropDownList;
                cmbViewModeDefault.Location = new Point(15, 40);
                cmbViewModeDefault.Width = 180;
                cmbViewModeDefault.SelectedIndexChanged += ViewModeDefaultChanged;

                btnRevertViewModeDefault = new Button();
                btnRevertViewModeDefault.Text = "\u21BA";
                btnRevertViewModeDefault.Size = new System.Drawing.Size(28, 24);
                btnRevertViewModeDefault.Location = new Point(200, 40);
                btnRevertViewModeDefault.Click += RevertViewModeDefault_Click;

                var lblPagerHeight = new Label();
                lblPagerHeight.Text = "Pager Height";
                lblPagerHeight.AutoSize = true;
                lblPagerHeight.Location = new Point(15, 70);
                var ttPagerHeight = new ToolTip();
                ttPagerHeight.SetToolTip(lblPagerHeight, "The pager width is automatically calculated from this value and from the size\nof all available screen space as well as the amount of virtual desktops.");

                numericPagerHeight = new NumericUpDown();
                numericPagerHeight.Minimum = 1;
                numericPagerHeight.Maximum = 9999;
                numericPagerHeight.Increment = 1;
                numericPagerHeight.Location = new Point(15, 90);
                numericPagerHeight.Size = new System.Drawing.Size(150, 22);
                numericPagerHeight.ValueChanged += PagerHeightChanged;

                btnRevertPagerHeight = new Button();
                btnRevertPagerHeight.Text = "\u21BA";
                btnRevertPagerHeight.Size = new System.Drawing.Size(28, 24);
                btnRevertPagerHeight.Location = new Point(200, 90);
                btnRevertPagerHeight.Click += RevertPagerHeight_Click;

                grpRendering.Controls.AddRange(new Control[] { lblViewModeDefault, cmbViewModeDefault, btnRevertViewModeDefault, lblPagerHeight, numericPagerHeight, btnRevertPagerHeight });
                scrollLayout.Controls.Add(grpRendering);
            }

            // GroupBox: Icons Settings
            {
                var grpIcons = new GroupBox();
                grpIcons.Text = "Icons Settings";
                grpIcons.Location = new Point(8, 115);
                grpIcons.Size = new System.Drawing.Size(420, 80);

                var lblIconPaddingX = new Label();
                lblIconPaddingX.Text = "Icon Padding X";
                lblIconPaddingX.AutoSize = true;
                lblIconPaddingX.Location = new Point(15, 20);
                var ttIconPaddingX = new ToolTip();
                ttIconPaddingX.SetToolTip(lblIconPaddingX, "Negative values are supported as well so less space is taken and the icons look stacked.");

                numericIconPaddingX = new NumericUpDown();
                numericIconPaddingX.Minimum = -9999;
                numericIconPaddingX.Maximum = 9999;
                numericIconPaddingX.Increment = 1;
                numericIconPaddingX.Location = new Point(15, 40);
                numericIconPaddingX.Size = new System.Drawing.Size(150, 22);
                numericIconPaddingX.ValueChanged += IconPaddingXChanged;

                btnRevertIconPaddingX = new Button();
                btnRevertIconPaddingX.Text = "\u21BA";
                btnRevertIconPaddingX.Size = new System.Drawing.Size(28, 24);
                btnRevertIconPaddingX.Location = new Point(170, 40);
                btnRevertIconPaddingX.Click += RevertIconPaddingX_Click;

                var lblIconPaddingY = new Label();
                lblIconPaddingY.Text = "Icon Padding Y";
                lblIconPaddingY.AutoSize = true;
                lblIconPaddingY.Location = new Point(15, 60);
                var ttIconPaddingY = new ToolTip();
                ttIconPaddingY.SetToolTip(lblIconPaddingY, "Negative values are supported as well in case of a very small pager height\nlike 40px which is usually the size of the Windows Taskbar.");

                numericIconPaddingY = new NumericUpDown();
                numericIconPaddingY.Minimum = -9999;
                numericIconPaddingY.Maximum = 9999;
                numericIconPaddingY.Increment = 1;
                numericIconPaddingY.Location = new Point(15, 80);
                numericIconPaddingY.Size = new System.Drawing.Size(150, 22);
                numericIconPaddingY.ValueChanged += IconPaddingYChanged;

                btnRevertIconPaddingY = new Button();
                btnRevertIconPaddingY.Text = "\u21BA";
                btnRevertIconPaddingY.Size = new System.Drawing.Size(28, 24);
                btnRevertIconPaddingY.Location = new Point(170, 80);
                btnRevertIconPaddingY.Click += RevertIconPaddingY_Click;

                grpIcons.Controls.AddRange(new Control[] { lblIconPaddingX, numericIconPaddingX, btnRevertIconPaddingX, lblIconPaddingY, numericIconPaddingY, btnRevertIconPaddingY });
                scrollLayout.Controls.Add(grpIcons);
            }

            // ============================ THEME TAB ============================
            // GroupBox: General
            {
                var grpGeneral = new GroupBox();
                grpGeneral.Text = "General";
                grpGeneral.Location = new Point(8, 8);
                grpGeneral.Width = 420;

                var lblOpacity = new Label();
                lblOpacity.Text = "App Background Opacity";
                lblOpacity.AutoSize = true;
                lblOpacity.Location = new Point(15, 20);

                sldBackgroundOpacity = new TrackBar();
                sldBackgroundOpacity.Minimum = 25;
                sldBackgroundOpacity.Maximum = 100;
                sldBackgroundOpacity.TickFrequency = 5;
                sldBackgroundOpacity.LargeChange = 10;
                sldBackgroundOpacity.Size = new System.Drawing.Size(100, 45);
                sldBackgroundOpacity.Location = new Point(15, 45);
                sldBackgroundOpacity.ValueChanged += BackgroundOpacityChanged;

                txtBackgroundOpacityValue = new Label();
                txtBackgroundOpacityValue.Text = "1.00";
                txtBackgroundOpacityValue.Location = new Point(120, 48);
                txtBackgroundOpacityValue.Size = new System.Drawing.Size(40, 20);

                btnRevertBackgroundOpacity = new Button();
                btnRevertBackgroundOpacity.Text = "\u21BA";
                btnRevertBackgroundOpacity.Size = new System.Drawing.Size(28, 24);
                btnRevertBackgroundOpacity.Location = new Point(168, 45);
                btnRevertBackgroundOpacity.Click += RevertBackgroundOpacity_Click;

                grpGeneral.Controls.AddRange(new Control[] { lblOpacity, sldBackgroundOpacity, txtBackgroundOpacityValue, btnRevertBackgroundOpacity });
                scrollTheme.Controls.Add(grpGeneral);
            }

            // GroupBox: Desktops
            {
                var grpDesktops = new GroupBox();
                grpDesktops.Text = "Desktops";
                grpDesktops.Location = new Point(8, 65);
                grpDesktops.Width = 420;

                // Row 0: Border Style
                var lblDesktopBorderStyle = new Label();
                lblDesktopBorderStyle.Text = "Border Style";
                lblDesktopBorderStyle.AutoSize = true;
                lblDesktopBorderStyle.Location = new Point(15, 20);

                cmbDesktopBorderStyle = new ComboBox();
                cmbDesktopBorderStyle.DropDownStyle = ComboBoxStyle.DropDownList;
                cmbDesktopBorderStyle.Location = new Point(15, 40);
                cmbDesktopBorderStyle.Width = 180;
                cmbDesktopBorderStyle.SelectedIndexChanged += DesktopBorderStyleChanged;

                btnRevertDesktopBorderStyle = new Button();
                btnRevertDesktopBorderStyle.Text = "\u21BA";
                btnRevertDesktopBorderStyle.Size = new System.Drawing.Size(28, 24);
                btnRevertDesktopBorderStyle.Location = new Point(220, 40);
                btnRevertDesktopBorderStyle.Click += RevertDesktopBorderStyle_Click;

                // Row 1: Background Color
                var lblBgColor = new Label();
                lblBgColor.Text = "Background Color";
                lblBgColor.AutoSize = true;
                lblBgColor.Location = new Point(15, 70);

                previewBackgroundColor = new Panel();
                previewBackgroundColor.Size = new System.Drawing.Size(32, 24);
                previewBackgroundColor.Location = new Point(150, 68);
                previewBackgroundColor.BorderStyle = BorderStyle.FixedSingle;

                var btnChooseBg = new Button();
                btnChooseBg.Text = "Choose...";
                btnChooseBg.Size = new System.Drawing.Size(72, 24);
                btnChooseBg.Location = new Point(190, 68);
                btnChooseBg.Click += ChooseDesktopBackgroundColor_Click;

                btnRevertBackgroundColor = new Button();
                btnRevertBackgroundColor.Text = "\u21BA";
                btnRevertBackgroundColor.Size = new System.Drawing.Size(28, 24);
                btnRevertBackgroundColor.Location = new Point(270, 68);
                btnRevertBackgroundColor.Click += RevertDesktopBackgroundColor_Click;

                // Row 2: Border Color
                var lblDsBorder = new Label();
                lblDsBorder.Text = "Border Color";
                lblDsBorder.AutoSize = true;
                lblDsBorder.Location = new Point(15, 100);

                previewDesktopColor = new Panel();
                previewDesktopColor.Size = new System.Drawing.Size(32, 24);
                previewDesktopColor.Location = new Point(150, 98);
                previewDesktopColor.BorderStyle = BorderStyle.FixedSingle;

                var btnChooseBorder = new Button();
                btnChooseBorder.Text = "Choose...";
                btnChooseBorder.Size = new System.Drawing.Size(72, 24);
                btnChooseBorder.Location = new Point(190, 98);
                btnChooseBorder.Click += ChooseDesktopBorderColor_Click;

                btnRevertDesktopColor = new Button();
                btnRevertDesktopColor.Text = "\u21BA";
                btnRevertDesktopColor.Size = new System.Drawing.Size(28, 24);
                btnRevertDesktopColor.Location = new Point(270, 98);
                btnRevertDesktopColor.Click += RevertDesktopBorderColor_Click;

                // Row 3: Active Border Color
                var lblActiveDsBorder = new Label();
                lblActiveDsBorder.Text = "Border Color (Active)";
                lblActiveDsBorder.AutoSize = true;
                lblActiveDsBorder.Location = new Point(15, 130);

                previewActiveDesktopBorderColor = new Panel();
                previewActiveDesktopBorderColor.Size = new System.Drawing.Size(32, 24);
                previewActiveDesktopBorderColor.Location = new Point(150, 128);
                previewActiveDesktopBorderColor.BorderStyle = BorderStyle.FixedSingle;

                var btnChooseActiveBorder = new Button();
                btnChooseActiveBorder.Text = "Choose...";
                btnChooseActiveBorder.Size = new System.Drawing.Size(72, 24);
                btnChooseActiveBorder.Location = new Point(190, 128);
                btnChooseActiveBorder.Click += ChooseActiveDesktopBorderColor_Click;

                btnRevertActiveDesktopBorderColor = new Button();
                btnRevertActiveDesktopBorderColor.Text = "\u21BA";
                btnRevertActiveDesktopBorderColor.Size = new System.Drawing.Size(28, 24);
                btnRevertActiveDesktopBorderColor.Location = new Point(270, 128);
                btnRevertActiveDesktopBorderColor.Click += RevertActiveDesktopBorderColor_Click;

                grpDesktops.Controls.AddRange(new Control[]
                {
                    lblDesktopBorderStyle, cmbDesktopBorderStyle, btnRevertDesktopBorderStyle,
                    lblBgColor, previewBackgroundColor, btnChooseBg, btnRevertBackgroundColor,
                    lblDsBorder, previewDesktopColor, btnChooseBorder, btnRevertDesktopColor,
                    lblActiveDsBorder, previewActiveDesktopBorderColor, btnChooseActiveBorder, btnRevertActiveDesktopBorderColor,
                });
                scrollTheme.Controls.Add(grpDesktops);
            }

            // GroupBox: Preview Windows
            {
                var grpWindows = new GroupBox();
                grpWindows.Text = "Preview Windows";
                grpWindows.Location = new Point(8, 355);
                grpWindows.Width = 420;

                // Row 0: Background Color
                var lblWinBg = new Label();
                lblWinBg.Text = "Background Color";
                lblWinBg.AutoSize = true;
                lblWinBg.Location = new Point(15, 20);

                previewWindowColor = new Panel();
                previewWindowColor.Size = new System.Drawing.Size(32, 24);
                previewWindowColor.Location = new Point(150, 18);
                previewWindowColor.BorderStyle = BorderStyle.FixedSingle;

                var btnChooseWinBg = new Button();
                btnChooseWinBg.Text = "Choose...";
                btnChooseWinBg.Size = new System.Drawing.Size(72, 24);
                btnChooseWinBg.Location = new Point(190, 18);
                btnChooseWinBg.Click += ChooseWindowBackgroundColor_Click;

                btnRevertWindowColor = new Button();
                btnRevertWindowColor.Text = "\u21BA";
                btnRevertWindowColor.Size = new System.Drawing.Size(28, 24);
                btnRevertWindowColor.Location = new Point(270, 18);
                btnRevertWindowColor.Click += RevertWindowBackgroundColor_Click;

                // Row 1: Border Color
                var lblWinBorder = new Label();
                lblWinBorder.Text = "Border Color";
                lblWinBorder.AutoSize = true;
                lblWinBorder.Location = new Point(15, 50);

                previewWindowBorderColor = new Panel();
                previewWindowBorderColor.Size = new System.Drawing.Size(32, 24);
                previewWindowBorderColor.Location = new Point(150, 48);
                previewWindowBorderColor.BorderStyle = BorderStyle.FixedSingle;

                var btnChooseWinBorder = new Button();
                btnChooseWinBorder.Text = "Choose...";
                btnChooseWinBorder.Size = new System.Drawing.Size(72, 24);
                btnChooseWinBorder.Location = new Point(190, 48);
                btnChooseWinBorder.Click += ChooseWindowBorderColor_Click;

                btnRevertWindowBorderColor = new Button();
                btnRevertWindowBorderColor.Text = "\u21BA";
                btnRevertWindowBorderColor.Size = new System.Drawing.Size(28, 24);
                btnRevertWindowBorderColor.Location = new Point(270, 48);
                btnRevertWindowBorderColor.Click += RevertWindowBorderColor_Click;

                // Row 2: Active Border Color
                var lblWinActiveBorder = new Label();
                lblWinActiveBorder.Text = "Border Color (Active)";
                lblWinActiveBorder.AutoSize = true;
                lblWinActiveBorder.Location = new Point(15, 80);

                previewActiveWindowBorderColor = new Panel();
                previewActiveWindowBorderColor.Size = new System.Drawing.Size(32, 24);
                previewActiveWindowBorderColor.Location = new Point(150, 78);
                previewActiveWindowBorderColor.BorderStyle = BorderStyle.FixedSingle;

                var btnChooseWinActiveBorder = new Button();
                btnChooseWinActiveBorder.Text = "Choose...";
                btnChooseWinActiveBorder.Size = new System.Drawing.Size(72, 24);
                btnChooseWinActiveBorder.Location = new Point(190, 78);
                btnChooseWinActiveBorder.Click += ChooseActiveWindowBorderColor_Click;

                btnRevertActiveWindowBorderColor = new Button();
                btnRevertActiveWindowBorderColor.Text = "\u21BA";
                btnRevertActiveWindowBorderColor.Size = new System.Drawing.Size(28, 24);
                btnRevertActiveWindowBorderColor.Location = new Point(270, 78);
                btnRevertActiveWindowBorderColor.Click += RevertActiveWindowBorderColor_Click;

                grpWindows.Controls.AddRange(new Control[]
                {
                    lblWinBg, previewWindowColor, btnChooseWinBg, btnRevertWindowColor,
                    lblWinBorder, previewWindowBorderColor, btnChooseWinBorder, btnRevertWindowBorderColor,
                    lblWinActiveBorder, previewActiveWindowBorderColor, btnChooseWinActiveBorder, btnRevertActiveWindowBorderColor,
                });
                scrollTheme.Controls.Add(grpWindows);
            }

            // GroupBox: Active Windows
            {
                var grpIconsActive = new GroupBox();
                grpIconsActive.Text = "Active Windows";
                grpIconsActive.Location = new Point(8, 505);
                grpIconsActive.Width = 420;

                var lblActiveBg = new Label();
                lblActiveBg.Text = "Background Color (Active)";
                lblActiveBg.AutoSize = true;
                lblActiveBg.Location = new Point(15, 20);

                previewActiveWindowColor = new Panel();
                previewActiveWindowColor.Size = new System.Drawing.Size(32, 24);
                previewActiveWindowColor.Location = new Point(150, 18);
                previewActiveWindowColor.BorderStyle = BorderStyle.FixedSingle;

                var btnChooseActiveBg = new Button();
                btnChooseActiveBg.Text = "Choose...";
                btnChooseActiveBg.Size = new System.Drawing.Size(72, 24);
                btnChooseActiveBg.Location = new Point(190, 18);
                btnChooseActiveBg.Click += ChooseActiveWindowBackgroundColor_Click;

                btnRevertActiveWindowColor = new Button();
                btnRevertActiveWindowColor.Text = "\u21BA";
                btnRevertActiveWindowColor.Size = new System.Drawing.Size(28, 24);
                btnRevertActiveWindowColor.Location = new Point(270, 18);
                btnRevertActiveWindowColor.Click += RevertActiveWindowBackgroundColor_Click;

                grpIconsActive.Controls.AddRange(new Control[] { lblActiveBg, previewActiveWindowColor, btnChooseActiveBg, btnRevertActiveWindowColor });
                scrollTheme.Controls.Add(grpIconsActive);
            }

            // ============================ MISC TAB ============================
            // GroupBox: Performance
            {
                var grpPerformance = new GroupBox();
                grpPerformance.Text = "Performance";
                grpPerformance.Location = new Point(8, 8);
                grpPerformance.Size = new System.Drawing.Size(420, 130);

                var lblPrimaryDelay = new Label();
                lblPrimaryDelay.Text = "Primary Update Delay (ms)";
                lblPrimaryDelay.AutoSize = true;
                lblPrimaryDelay.Location = new Point(15, 20);
                var ttPrimaryDelay = new ToolTip();
                ttPrimaryDelay.SetToolTip(lblPrimaryDelay, "Delay for the main refresh cycle.\nAffects how often the currently opened windows\n(on all virtual desktops) are checked for changes.\n\nLower values update the pager faster, but can increase CPU usage.");

                txtPrimaryDelay = new TextBox();
                txtPrimaryDelay.Size = new System.Drawing.Size(150, 22);
                txtPrimaryDelay.Location = new Point(15, 40);
                txtPrimaryDelay.TextChanged += MiscTextInputChanged;

                btnRevertPrimaryDelay = new Button();
                btnRevertPrimaryDelay.Text = "\u21BA";
                btnRevertPrimaryDelay.Size = new System.Drawing.Size(28, 24);
                btnRevertPrimaryDelay.Location = new Point(170, 40);
                btnRevertPrimaryDelay.Click += RevertPrimaryDelay_Click;

                var lblSecondaryDelay = new Label();
                lblSecondaryDelay.Text = "Secondary Update Delay (ms)";
                lblSecondaryDelay.AutoSize = true;
                lblSecondaryDelay.Location = new Point(15, 70);
                var ttSecondaryDelay = new ToolTip();
                ttSecondaryDelay.SetToolTip(lblSecondaryDelay, "Delay for background/secondary updates.\nAffects how often the app window is checked for being covered by another one,\nthis includes the Windows Taskbar. Also (re-)pins the app to all virtual desktops.\n\nSome of those operations require more processing,\nthat's why this delay should be higher than the primary one.");

                txtSecondaryDelay = new TextBox();
                txtSecondaryDelay.Size = new System.Drawing.Size(150, 22);
                txtSecondaryDelay.Location = new Point(15, 90);
                txtSecondaryDelay.TextChanged += MiscTextInputChanged;

                btnRevertSecondaryDelay = new Button();
                btnRevertSecondaryDelay.Text = "\u21BA";
                btnRevertSecondaryDelay.Size = new System.Drawing.Size(28, 24);
                btnRevertSecondaryDelay.Location = new Point(170, 90);
                btnRevertSecondaryDelay.Click += RevertSecondaryDelay_Click;

                grpPerformance.Controls.AddRange(new Control[] { lblPrimaryDelay, txtPrimaryDelay, btnRevertPrimaryDelay, lblSecondaryDelay, txtSecondaryDelay, btnRevertSecondaryDelay });
                scrollMisc.Controls.Add(grpPerformance);
            }

            // GroupBox: App Pinning
            {
                var grpPinning = new GroupBox();
                grpPinning.Text = "App Pinning";
                grpPinning.Location = new Point(8, 145);
                grpPinning.Width = 420;

                var lblShowInTaskbar = new Label();
                lblShowInTaskbar.Text = "Show App In Taskbar";
                lblShowInTaskbar.AutoSize = true;
                lblShowInTaskbar.Location = new Point(15, 20);

                chkShowAppInTaskbar = new CheckBox();
                chkShowAppInTaskbar.CheckedChanged += MiscBooleanInputChanged;

                btnRevertShowAppInTaskbar = new Button();
                btnRevertShowAppInTaskbar.Text = "\u21BA";
                btnRevertShowAppInTaskbar.Size = new System.Drawing.Size(28, 24);
                btnRevertShowAppInTaskbar.Location = new Point(170, 18);
                btnRevertShowAppInTaskbar.Click += RevertShowAppInTaskbar_Click;

                var lblNote = new Label();
                lblNote.Text = "Important: When deactivated, the automatic pinning of the app (to be visible on all virtual desktops) can't work anymore because of a limitation of the Windows OS. So the pinning has to be done manually after every application start using the \"Active Applications\" screen (Win + Tab).";
                lblNote.AutoSize = false;
                lblNote.Width = 390;
                lblNote.Top = 50;
                lblNote.Left = 15;
                lblNote.ForeColor = SystemColors.GrayText;
                lblNote.TextAlign = ContentAlignment.MiddleLeft;

                var panelNoteContainer = new Panel();
                panelNoteContainer.BorderStyle = BorderStyle.FixedSingle;
                panelNoteContainer.Size = new System.Drawing.Size(400, 50);
                panelNoteContainer.Left = 15;
                panelNoteContainer.Top = 48;
                panelNoteContainer.Controls.Add(lblNote);

                grpPinning.Controls.AddRange(new Control[] { lblShowInTaskbar, chkShowAppInTaskbar, btnRevertShowAppInTaskbar, panelNoteContainer });
                scrollMisc.Controls.Add(grpPinning);
            }

            // Add tab control and button bar to the form
            Controls.Add(tabControl);
            Controls.Add(buttonFlowLayoutPanel);

            // Position the form next to the owner if available
            if (_hasPlacementOwner)
            {
                var size = this.Size;
                DrawingPoint pt = FindPlacement(_placementOwnerBounds, size);
                Location = new System.Drawing.Point(pt.X, pt.Y);
            }

            ResumeLayout(false);
        }

        #region Placement helpers (WinForms equivalent of WPF version)
        private static DrawingPoint FindPlacement(DrawingRectangle owner, DrawingSize size)
        {
            return TryPlaceRight(owner, size)
                ?? TryPlaceLeft(owner, size)
                ?? TryPlaceBelow(owner, size)
                ?? TryPlaceAbove(owner, size)
                ?? CenterOverOwner(owner, size);
        }

        private static DrawingPoint? TryPlaceRight(DrawingRectangle owner, DrawingSize size)
        {
            int x = owner.Right + PlacementGap;
            foreach (var screen in Screen.AllScreens)
            {
                var area = screen.WorkingArea;
                if (x >= area.Left && x + size.Width <= area.Right && size.Height <= area.Height)
                {
                    int y = Math.Max(area.Top, Math.Min(owner.Top, area.Bottom - size.Height));
                    return new DrawingPoint(x, y);
                }
            }

            return null;
        }

        private static DrawingPoint? TryPlaceLeft(DrawingRectangle owner, DrawingSize size)
        {
            int x = owner.Left - PlacementGap - size.Width;
            foreach (var screen in Screen.AllScreens)
            {
                var area = screen.WorkingArea;
                if (x >= area.Left && x + size.Width <= area.Right && size.Height <= area.Height)
                {
                    int y = Math.Max(area.Top, Math.Min(owner.Top, area.Bottom - size.Height));
                    return new DrawingPoint(x, y);
                }
            }

            return null;
        }

        private static DrawingPoint? TryPlaceBelow(DrawingRectangle owner, DrawingSize size)
        {
            int y = owner.Bottom + PlacementGap;
            foreach (var screen in Screen.AllScreens)
            {
                var area = screen.WorkingArea;
                if (y >= area.Top && y + size.Height <= area.Bottom && size.Width <= area.Width)
                {
                    int x = Math.Max(area.Left, Math.Min(owner.Left, area.Right - size.Width));
                    return new DrawingPoint(x, y);
                }
            }

            return null;
        }

        private static DrawingPoint? TryPlaceAbove(DrawingRectangle owner, DrawingSize size)
        {
            int y = owner.Top - PlacementGap - size.Height;
            foreach (var screen in Screen.AllScreens)
            {
                var area = screen.WorkingArea;
                if (y >= area.Top && y + size.Height <= area.Bottom && size.Width <= area.Width)
                {
                    int x = Math.Max(area.Left, Math.Min(owner.Left, area.Right - size.Width));
                    return new DrawingPoint(x, y);
                }
            }
            
            return null;
        }

        private static DrawingPoint CenterOverOwner(DrawingRectangle owner, DrawingSize size)
        {
            return new DrawingPoint(
                owner.Left + ((owner.Width - size.Width) / 2),
                owner.Top + ((owner.Height - size.Height) / 2));
        }
        #endregion
    }
}
