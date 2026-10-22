using System;
using System.Threading;
using System.Windows.Forms;
using MicroTaskPager.Core;

namespace MicroTaskPager
{
    internal static class Program
    {
        private static Mutex mutex = null;

        public static CancellationToken ApplicationClosing = new CancellationToken();
        public static WindowsVersion WindowsVersion = new WindowsVersion();

        [STAThread]
        private static void Main()
        {
            // Only allow a single instance of the app per windows user
            const string mutexName = "micro-task-pager-win-app";
            mutex = new Mutex(true, mutexName, out bool createdNew);
            if (!createdNew)
            {
                MessageBox.Show("Micro Task Pager is already running.", "Startup Error");
                return;
            }

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new GUI.MainForm());

            // Release mutex before quitting the app
            mutex.ReleaseMutex();
        }
    }
}
