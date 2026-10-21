using System;
using System.Drawing;
using System.Globalization;
using Microsoft.Win32;

namespace Switchie.GUI
{
    public static class AppSettingsStore
    {
        private const string RegistryKeyPath = @"SOFTWARE\Switchie";

        public static AppSettings Load()
        {
            var defaults = new AppSettings();
            using (RegistryKey key = Registry.CurrentUser.OpenSubKey(RegistryKeyPath))
            {
                return key == null ? defaults : new AppSettings
                {
                    DesktopModeDefault = (DesktopMode)ReadInt(key, "DesktopModeDefault", (int)defaults.DesktopModeDefault),
                    DesktopBorderStyle = (BorderStyle)ReadInt(key, "DesktopBorderStyle", (int)defaults.DesktopBorderStyle),
                    BackgroundOpacity = ReadDouble(key, "BackgroundOpacity", defaults.BackgroundOpacity),
                    PagerHeight = ReadInt(key, "PagerHeight", defaults.PagerHeight),
                    IconPaddingX = ReadInt(key, "IconPaddingX", defaults.IconPaddingX),
                    IconPaddingY = ReadInt(key, "IconPaddingY", defaults.IconPaddingY),

                    DesktopBackgroundColor = ReadColor(key, "DesktopBackgroundColor", defaults.DesktopBackgroundColor),

                    DesktopBorderColor = ReadColor(key, "DesktopBorderColor", defaults.DesktopBorderColor),
                    ActiveDesktopBorderColor = ReadColor(key, "ActiveDesktopBorderColor", defaults.ActiveDesktopBorderColor),

                    WindowBackgroundColor = ReadColor(key, "WindowBackgroundColor", defaults.WindowBackgroundColor),
                    ActiveWindowBackgroundColor = ReadColor(key, "ActiveWindowBackgroundColor", defaults.ActiveWindowBackgroundColor),
                    WindowBorderColor = ReadColor(key, "WindowBorderColor", defaults.WindowBorderColor),
                    ActiveWindowBorderColor = ReadColor(key, "ActiveWindowBorderColor", defaults.ActiveWindowBorderColor),

                    PrimaryUpdateDelay = ReadInt(key, "PrimaryUpdateDelay", defaults.PrimaryUpdateDelay),
                    SecondaryUpdateDelay = ReadInt(key, "SecondaryUpdateDelay", defaults.SecondaryUpdateDelay),
                    ShowAppInTaskbar = ReadInt(key, "ShowAppInTaskbar", defaults.ShowAppInTaskbar ? 1 : 0) == 1,
                };
            }
        }

        public static void Save(AppSettings settings)
        {
            using (RegistryKey key = Registry.CurrentUser.CreateSubKey(RegistryKeyPath))
            {
                if (key == null) return;
                key.SetValue("DesktopModeDefault", settings.DesktopModeDefault, RegistryValueKind.DWord);
                key.SetValue("DesktopBorderStyle", settings.DesktopBorderStyle, RegistryValueKind.DWord);
                key.SetValue("BackgroundOpacity", settings.BackgroundOpacity.ToString(CultureInfo.InvariantCulture), RegistryValueKind.String);
                key.SetValue("PagerHeight", settings.PagerHeight, RegistryValueKind.DWord);
                key.SetValue("IconPaddingX", settings.IconPaddingX, RegistryValueKind.DWord);
                key.SetValue("IconPaddingY", settings.IconPaddingY, RegistryValueKind.DWord);

                key.SetValue("DesktopBackgroundColor", settings.DesktopBackgroundColor.ToArgb(), RegistryValueKind.DWord);

                key.SetValue("DesktopBorderColor", settings.DesktopBorderColor.ToArgb(), RegistryValueKind.DWord);
                key.SetValue("ActiveDesktopBorderColor", settings.ActiveDesktopBorderColor.ToArgb(), RegistryValueKind.DWord);

                key.SetValue("WindowBackgroundColor", settings.WindowBackgroundColor.ToArgb(), RegistryValueKind.DWord);
                key.SetValue("ActiveWindowBackgroundColor", settings.ActiveWindowBackgroundColor.ToArgb(), RegistryValueKind.DWord);
                key.SetValue("WindowBorderColor", settings.WindowBorderColor.ToArgb(), RegistryValueKind.DWord);
                key.SetValue("ActiveWindowBorderColor", settings.ActiveWindowBorderColor.ToArgb(), RegistryValueKind.DWord);

                key.SetValue("PrimaryUpdateDelay", settings.PrimaryUpdateDelay, RegistryValueKind.DWord);
                key.SetValue("SecondaryUpdateDelay", settings.SecondaryUpdateDelay, RegistryValueKind.DWord);
                key.SetValue("ShowAppInTaskbar", settings.ShowAppInTaskbar ? 1 : 0, RegistryValueKind.DWord);
            }
        }

        private static int ReadInt(RegistryKey key, string valueName, int fallback)
        {
            object value = key.GetValue(valueName);
            return value == null ? fallback : int.TryParse(value.ToString(), out int result) ? result : fallback;
        }

        private static Color ReadColor(RegistryKey key, string valueName, Color fallback)
        {
            object value = key.GetValue(valueName);
            return value == null ? fallback : int.TryParse(value.ToString(), out int argb) ? Color.FromArgb(argb) : fallback;
        }

        private static double ReadDouble(RegistryKey key, string valueName, double fallback)
        {
            object value = key.GetValue(valueName);
            return value == null
                ? fallback
                : double.TryParse(value.ToString(), NumberStyles.Float | NumberStyles.AllowThousands, CultureInfo.InvariantCulture, out double result)
                    ? result
                    : fallback;
        }
    }
}