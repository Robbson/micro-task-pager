namespace MicroTaskPager
{
    public class WindowsVersion
    {
        public WindowsVersion()
        {
            Microsoft.Win32.RegistryKey registryKey = Microsoft.Win32.Registry.LocalMachine.OpenSubKey(@"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
            Major = int.Parse(registryKey.GetValue("CurrentMajorVersionNumber").ToString());
            Minor = int.Parse(registryKey.GetValue("CurrentMinorVersionNumber").ToString());
            Build = int.Parse(registryKey.GetValue("CurrentBuildNumber").ToString());
        }

        public int Major { get; set; }
        public int Minor { get; set; }
        public int Build { get; set; }

        public bool IsWin11_22H2_OrLater() => Major == 10 && Minor == 0 && Build >= 22621;
        public bool IsWin11_21H2_OrLater() => Major == 10 && Minor == 0 && Build >= 22000;
        public bool IsWin10() => Major == 10 && Minor == 0 && Build > 17763 && Build < 22000;
        public bool IsWin10LTSC() => Major == 10 && Minor == 0 && Build <= 17763;
    }
}