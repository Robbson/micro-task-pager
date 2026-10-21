using System;
using System.Diagnostics;
using System.Globalization;
using System.Reflection.Emit;
using System.Windows;
using System.Windows.Controls;
using Switchie.GUI;
using DrawingColor = System.Drawing.Color;
using Forms = System.Windows.Forms;

namespace Switchie
{
    public partial class AppSettingsWindow : System.Windows.Window
    {
        private readonly AppSettings _defaults = new AppSettings();
        private readonly AppSettings _settingsBefore;

        private DrawingColor _desktopBackgroundColor;
        private DrawingColor _desktopBorderColor;
        private DrawingColor _activeDesktopBorderColor;
        private DrawingColor _windowBackgroundColor;
        private DrawingColor _activeWindowBackgroundColor;
        private DrawingColor _windowBorderColor;
        private DrawingColor _activeWindowBorderColor;

        public AppSettingsWindow(AppSettings settings)
        {
            InitializeComponent();

            _settingsBefore = AppSettings.Clone(settings);
            Settings = AppSettings.Clone(settings);

            cmbDesktopModeDefault.Items.Add("Preview");
            cmbDesktopModeDefault.Items.Add("MicroTaskbar");
            cmbDesktopModeDefault.SelectedIndex = (int)Settings.DesktopModeDefault; //Math.Max(0, Math.Min(1, Settings.DesktopModeDefault));
            numericPagerHeight.Value = Settings.PagerHeight;
            numericIconPaddingX.Value = Settings.IconPaddingX;
            numericIconPaddingY.Value = Settings.IconPaddingY;

            if (sldBackgroundOpacity != null)
            {
                sldBackgroundOpacity.Value = Math.Max(0.25, Math.Min(1.0, Settings.BackgroundOpacity));
            }

            if (txtBackgroundOpacityValue != null && sldBackgroundOpacity != null)
            {
                txtBackgroundOpacityValue.Text = sldBackgroundOpacity.Value.ToString("0.00", CultureInfo.InvariantCulture);
            }

            cmbDesktopBorderStyle.Items.Add("Box");
            cmbDesktopBorderStyle.Items.Add("Underline");
            cmbDesktopBorderStyle.SelectedIndex = (int)Settings.DesktopBorderStyle; //Math.Max(0, Math.Min(1, Settings.DesktopBorderStyle));
            _desktopBackgroundColor = Settings.DesktopBackgroundColor;
            _desktopBorderColor = Settings.DesktopBorderColor;
            _activeDesktopBorderColor = Settings.ActiveDesktopBorderColor;
            _windowBackgroundColor = Settings.WindowBackgroundColor;
            _activeWindowBackgroundColor = Settings.ActiveWindowBackgroundColor;
            _windowBorderColor = Settings.WindowBorderColor;
            _activeWindowBorderColor = Settings.ActiveWindowBorderColor;

            txtPrimaryDelay.Text = Settings.PrimaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
            txtSecondaryDelay.Text = Settings.SecondaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
            chkShowAppInTaskbar.IsChecked = Settings.ShowAppInTaskbar;

            RefreshColorPreviews();
            UpdateRevertButtons();
        }

        public event Action<AppSettings> PreviewChanged;

        public AppSettings Settings { get; private set; }

        #region Generic arsers & utilites
        private static bool TryParsePositiveInt(string input, int min, out int value)
        {
            return int.TryParse(input, NumberStyles.Integer, CultureInfo.InvariantCulture, out value) && value >= min;
        }

        private static bool TryParseInt(string input, out int value)
        {
            return int.TryParse(input, NumberStyles.Integer, CultureInfo.InvariantCulture, out value);
        }

        private static bool IsDifferent(string input, int defaultValue)
        {
            return !int.TryParse(input, NumberStyles.Integer, CultureInfo.InvariantCulture, out int parsed) || parsed != defaultValue;
        }

        private static bool IsDifferent(int input, int defaultValue)
        {
            return input != defaultValue;
        }

        private static bool IsDifferent(double input, double defaultValue)
        {
            return Math.Abs(input - defaultValue) > 0.0001;
        }
        #endregion

        private void ResetToDefaults_Click(object sender, RoutedEventArgs e)
        {
            if (tabControl.SelectedIndex == 0)
            {
                cmbDesktopModeDefault.SelectedIndex = _defaults.DesktopModeDefault <= 0 ? 0 : 1;
                numericPagerHeight.Value = _defaults.PagerHeight;
                numericIconPaddingX.Value = _defaults.IconPaddingX;
                numericIconPaddingY.Value = _defaults.IconPaddingY;
            }
            else if (tabControl.SelectedIndex == 1)
            {
                if (sldBackgroundOpacity != null)
                {
                    sldBackgroundOpacity.Value = Math.Max(0.25, Math.Min(1.0, _defaults.BackgroundOpacity));
                }

                cmbDesktopBorderStyle.SelectedIndex = _defaults.DesktopBorderStyle <= 0 ? 0 : 1;
                _desktopBackgroundColor = _defaults.DesktopBackgroundColor;
                _desktopBorderColor = _defaults.DesktopBorderColor;
                _activeDesktopBorderColor = _defaults.ActiveDesktopBorderColor;
                _windowBackgroundColor = _defaults.WindowBackgroundColor;
                _activeWindowBackgroundColor = _defaults.ActiveWindowBackgroundColor;
                _windowBorderColor = _defaults.WindowBorderColor;
                _activeWindowBorderColor = _defaults.ActiveWindowBorderColor;
                RequestPreview();
            }
            else if (tabControl.SelectedIndex == 2)
            {
                txtPrimaryDelay.Text = _defaults.PrimaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
                txtSecondaryDelay.Text = _defaults.SecondaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
                chkShowAppInTaskbar.IsChecked = _defaults.ShowAppInTaskbar;
            }

            RefreshColorPreviews();
            UpdateRevertButtons();
        }

        private void ShowValidationError(string message)
        {
            MessageBox.Show(this, message, "Invalid Settings", MessageBoxButton.OK, MessageBoxImage.Warning);
        }

        private AppSettings BuildSettingsFromControls()
        {
            // Because initial (not) set values can trigger previews with invalid values
            if (!IsLoaded) return null;
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

            return new AppSettings
            {
                DesktopModeDefault = (DesktopMode)(cmbDesktopModeDefault.SelectedIndex <= 0 ? 0 : 1),
                PagerHeight = numericPagerHeight.Value,
                IconPaddingX = numericIconPaddingX.Value,
                IconPaddingY = numericIconPaddingY.Value,

                BackgroundOpacity = sldBackgroundOpacity == null
                    ? Math.Max(0.25, Math.Min(1.0, _defaults.BackgroundOpacity))
                    : Math.Max(0.25, Math.Min(1.0, sldBackgroundOpacity.Value)),
                DesktopBorderStyle = (BorderStyle)(cmbDesktopBorderStyle.SelectedIndex <= 0 ? 0 : 1),
                DesktopBackgroundColor = _desktopBackgroundColor,
                DesktopBorderColor = _desktopBorderColor,
                ActiveDesktopBorderColor = _activeDesktopBorderColor,
                WindowBackgroundColor = _windowBackgroundColor,
                ActiveWindowBackgroundColor = _activeWindowBackgroundColor,
                WindowBorderColor = _windowBorderColor,
                ActiveWindowBorderColor = _activeWindowBorderColor,

                PrimaryUpdateDelay = primaryDelay,
                SecondaryUpdateDelay = secondaryDelay,
                ShowAppInTaskbar = chkShowAppInTaskbar.IsChecked == true,
            };
        }

        // TODO: Do I still need this Event when I know modify the options directly
        // -> Well, for recalculations it s necessary
        private void RequestPreview()
        {
            // We don't need to pass the settings anymore if we can manipulate the settings directly (what happened by PreviewChanged before anyway)
            var s = BuildSettingsFromControls();

            if (s != null)
            {
                //Settings = s;
                PreviewChanged?.Invoke(s);   // invalid input: keep last good preview
            }
        }

        private void Save_Click(object sender, RoutedEventArgs e)
        {
            Settings = BuildSettingsFromControls();
            if (Settings == null) return;
            DialogResult = true;
            Close();
        }

        private void UpdateRevertButtons()
        {
            if (btnRevertBackgroundOpacity == null || sldBackgroundOpacity == null)
            {
                return;
            }

            btnRevertDesktopModeDefault.Visibility = cmbDesktopModeDefault.SelectedIndex != (int)_settingsBefore.DesktopModeDefault ? Visibility.Visible : Visibility.Hidden;
            btnRevertPagerHeight.Visibility = numericPagerHeight.Value != _settingsBefore.PagerHeight ? Visibility.Visible : Visibility.Hidden;
            btnRevertIconPaddingX.Visibility = numericIconPaddingX.Value != _settingsBefore.IconPaddingX ? Visibility.Visible : Visibility.Hidden;
            btnRevertIconPaddingY.Visibility = numericIconPaddingY.Value != _settingsBefore.IconPaddingY ? Visibility.Visible : Visibility.Hidden;

            btnRevertBackgroundOpacity.Visibility = IsDifferent(sldBackgroundOpacity.Value, _settingsBefore.BackgroundOpacity) ? Visibility.Visible : Visibility.Hidden;
            btnRevertDesktopBorderStyle.Visibility = cmbDesktopBorderStyle.SelectedIndex != (int)_settingsBefore.DesktopBorderStyle ? Visibility.Visible : Visibility.Hidden;
            btnRevertBackgroundColor.Visibility = _desktopBackgroundColor.ToArgb() != _settingsBefore.DesktopBackgroundColor.ToArgb() ? Visibility.Visible : Visibility.Hidden;
            btnRevertDesktopColor.Visibility = _desktopBorderColor.ToArgb() != _settingsBefore.DesktopBorderColor.ToArgb() ? Visibility.Visible : Visibility.Hidden;
            btnRevertActiveDesktopBorderColor.Visibility = _activeDesktopBorderColor.ToArgb() != _settingsBefore.ActiveDesktopBorderColor.ToArgb() ? Visibility.Visible : Visibility.Hidden;
            btnRevertWindowColor.Visibility = _windowBackgroundColor.ToArgb() != _settingsBefore.WindowBackgroundColor.ToArgb() ? Visibility.Visible : Visibility.Hidden;
            btnRevertActiveWindowColor.Visibility = _activeWindowBackgroundColor.ToArgb() != _settingsBefore.ActiveWindowBackgroundColor.ToArgb() ? Visibility.Visible : Visibility.Hidden;
            btnRevertWindowBorderColor.Visibility = _windowBorderColor.ToArgb() != _settingsBefore.WindowBorderColor.ToArgb() ? Visibility.Visible : Visibility.Hidden;
            btnRevertActiveWindowBorderColor.Visibility = _activeWindowBorderColor.ToArgb() != _settingsBefore.ActiveWindowBorderColor.ToArgb() ? Visibility.Visible : Visibility.Hidden;

            btnRevertPrimaryDelay.Visibility = IsDifferent(txtPrimaryDelay.Text, _settingsBefore.PrimaryUpdateDelay) ? Visibility.Visible : Visibility.Hidden;
            btnRevertSecondaryDelay.Visibility = IsDifferent(txtSecondaryDelay.Text, _settingsBefore.SecondaryUpdateDelay) ? Visibility.Visible : Visibility.Hidden;
            btnRevertShowAppInTaskbar.Visibility = (chkShowAppInTaskbar.IsChecked == true) != _settingsBefore.ShowAppInTaskbar ? Visibility.Visible : Visibility.Hidden;
        }

        #region Layout Tab Handlers
        private void DesktopModeDefaultChanged(object sender, SelectionChangedEventArgs e)
        {
            UpdateRevertButtons();
            RequestPreview();
        }

        private void RevertDesktopModeDefault_Click(object sender, RoutedEventArgs e)
        {
            cmbDesktopModeDefault.SelectedIndex = _settingsBefore.DesktopModeDefault <= 0 ? 0 : 1;
            UpdateRevertButtons();
        }

        // THIS is very special because it needs recalulation in the MainFrame
        private void PagerHeightChanged(object sender, RoutedPropertyChangedEventArgs<int> e)
        {
            UpdateRevertButtons();

            //TODO: Preview Suppoer with those notices from AI help
            RequestPreview();
        }

        private void RevertPagerHeight_Click(object sender, RoutedEventArgs e)
        {
            numericPagerHeight.Value = _settingsBefore.PagerHeight;
            UpdateRevertButtons();
            RequestPreview();
        }

        // For: IconPaddingX and IconPaddingY
        private void PaddingInputChanged(object sender, RoutedPropertyChangedEventArgs<int> e)
        {
            UpdateRevertButtons();
            RequestPreview();
        }

        // Why we don't need a call to RequestPreview() on those 2?
        private void RevertIconPaddingX_Click(object sender, RoutedEventArgs e)
        {
            numericIconPaddingX.Value = _settingsBefore.IconPaddingX;
            UpdateRevertButtons();
        }

        private void RevertIconPaddingY_Click(object sender, RoutedEventArgs e)
        {
            numericIconPaddingY.Value = _settingsBefore.IconPaddingY;
            UpdateRevertButtons();
        }
        #endregion

        #region Theme Tab Handlers
        private void BackgroundOpacityChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            //if (!IsLoaded) return;
            if (txtBackgroundOpacityValue != null)
            {
                txtBackgroundOpacityValue.Text = sldBackgroundOpacity.Value.ToString("0.00", CultureInfo.InvariantCulture);
            }

            UpdateRevertButtons();
            RequestPreview();
        }

        private void RevertBackgroundOpacity_Click(object sender, RoutedEventArgs e)
        {
            if (sldBackgroundOpacity != null)
            {
                sldBackgroundOpacity.Value = Math.Max(0.25, Math.Min(1.0, _settingsBefore.BackgroundOpacity));
            }

            UpdateRevertButtons();
            RequestPreview();
        }

        private void DesktopBorderStyleChanged(object sender, SelectionChangedEventArgs e)
        {
            UpdateRevertButtons();
            RequestPreview();
        }

        private void RevertDesktopBorderStyle_Click(object sender, RoutedEventArgs e)
        {
            cmbDesktopBorderStyle.SelectedIndex = _settingsBefore.DesktopBorderStyle <= 0 ? 0 : 1;
            UpdateRevertButtons();
            RequestPreview();
        }

        private void UpdateColorsTab()
        {
            RefreshColorPreviews();
            UpdateRevertButtons();
            RequestPreview();
        }

        private void ChooseColor(ref DrawingColor color)
        {
            color = PickColor(color);
            UpdateColorsTab();
        }

        private void RevertColor(ref DrawingColor color, DrawingColor beforeColor)
        {
            color = beforeColor;
            UpdateColorsTab();
        }

        private void ChooseDesktopBackgroundColor_Click(object sender, RoutedEventArgs e)
        {
            ChooseColor(ref _desktopBackgroundColor);
            Settings.DesktopBackgroundColor = _desktopBackgroundColor;
        }

        private void ChooseDesktopBorderColor_Click(object sender, RoutedEventArgs e)
        {
            ChooseColor(ref _desktopBorderColor);
        }

        private void ChooseActiveDesktopBorderColor_Click(object sender, RoutedEventArgs e)
        {
            ChooseColor(ref _activeDesktopBorderColor);
        }

        private void ChooseWindowBackgroundColor_Click(object sender, RoutedEventArgs e)
        {
            ChooseColor(ref _windowBackgroundColor);
        }

        private void ChooseActiveWindowBackgroundColor_Click(object sender, RoutedEventArgs e)
        {
            ChooseColor(ref _activeWindowBackgroundColor);
        }

        private void ChooseWindowBorderColor_Click(object sender, RoutedEventArgs e)
        {
            ChooseColor(ref _windowBorderColor);
        }

        private void ChooseActiveWindowBorderColor_Click(object sender, RoutedEventArgs e)
        {
            ChooseColor(ref _activeWindowBorderColor);
        }

        private void RevertDesktopBackgroundColor_Click(object sender, RoutedEventArgs e)
        {
            RevertColor(ref _desktopBackgroundColor, _settingsBefore.DesktopBackgroundColor);
        }

        private void RevertDesktopBorderColor_Click(object sender, RoutedEventArgs e)
        {
            RevertColor(ref _desktopBorderColor, _settingsBefore.DesktopBorderColor);
        }

        private void RevertActiveDesktopBorderColor_Click(object sender, RoutedEventArgs e)
        {
            RevertColor(ref _activeDesktopBorderColor, _settingsBefore.ActiveDesktopBorderColor);
        }

        private void RevertWindowBackgroundColor_Click(object sender, RoutedEventArgs e)
        {
            RevertColor(ref _windowBackgroundColor, _settingsBefore.WindowBackgroundColor);
        }

        private void RevertActiveWindowBackgroundColor_Click(object sender, RoutedEventArgs e)
        {
            RevertColor(ref _activeWindowBackgroundColor, _settingsBefore.ActiveWindowBackgroundColor);
        }

        private void RevertWindowBorderColor_Click(object sender, RoutedEventArgs e)
        {
            RevertColor(ref _windowBorderColor, _settingsBefore.WindowBorderColor);
        }

        private void RevertActiveWindowBorderColor_Click(object sender, RoutedEventArgs e)
        {
            RevertColor(ref _activeWindowBorderColor, _settingsBefore.ActiveWindowBorderColor);
        }

        private static DrawingColor PickColor(DrawingColor initial)
        {
            using (var dlg = new Forms.ColorDialog())
            {
                dlg.FullOpen = true;
                dlg.Color = initial;
                return dlg.ShowDialog() == Forms.DialogResult.OK ? dlg.Color : initial;
            }
        }

        private void RefreshColorPreviews()
        {
            previewBackgroundColor.Background = ToBrush(_desktopBackgroundColor);
            previewDesktopColor.Background = ToBrush(_desktopBorderColor);
            previewActiveDesktopBorderColor.Background = ToBrush(_activeDesktopBorderColor);
            previewWindowColor.Background = ToBrush(_windowBackgroundColor);
            previewActiveWindowColor.Background = ToBrush(_activeWindowBackgroundColor);
            previewWindowBorderColor.Background = ToBrush(_windowBorderColor);
            previewActiveWindowBorderColor.Background = ToBrush(_activeWindowBorderColor);
        }

        private static System.Windows.Media.SolidColorBrush ToBrush(DrawingColor color)
        {
            return new System.Windows.Media.SolidColorBrush(System.Windows.Media.Color.FromArgb(color.A, color.R, color.G, color.B));
        }
        #endregion

        #region Misc Tab Handlers
        private void MiscTextInputChanged(object sender, TextChangedEventArgs e)
        {
            UpdateRevertButtons();
        }

        private void MiscBooleanInputChanged(object sender, RoutedEventArgs e)
        {
            UpdateRevertButtons();
        }

        private void RevertPrimaryDelay_Click(object sender, RoutedEventArgs e)
        {
            txtPrimaryDelay.Text = _settingsBefore.PrimaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
            UpdateRevertButtons();
        }

        private void RevertSecondaryDelay_Click(object sender, RoutedEventArgs e)
        {
            txtSecondaryDelay.Text = _settingsBefore.SecondaryUpdateDelay.ToString(CultureInfo.InvariantCulture);
            UpdateRevertButtons();
        }

        private void RevertShowAppInTaskbar_Click(object sender, RoutedEventArgs e)
        {
            chkShowAppInTaskbar.IsChecked = _settingsBefore.ShowAppInTaskbar;
            UpdateRevertButtons();
        }
        #endregion
    }
}