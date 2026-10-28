using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;
using MicroTaskPager.Core;
using MicroTaskPager.GUI;
using Serilog;

namespace MicroTaskPager
{
    internal static class Program
    {
        public static readonly CancellationTokenSource ApplicationClosing = new CancellationTokenSource();

#pragma warning disable SA1401 // Fields should be private -> Needs to be public
        public static WindowsVersion WindowsVersion = new WindowsVersion();
#pragma warning restore SA1401 // Fields should be private

        private static Mutex mutex = null;

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

            SetupLogging();

            try
            {
                Log.Information("Application started...");

                // Register global exception handler
                SetupGlobalExceptionHandling();

                Application.SetHighDpiMode(HighDpiMode.SystemAware);
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                Application.Run(new GUI.MainForm());

                // Release mutex before quitting the app
                mutex.ReleaseMutex();
            }
            catch (Exception ex)
            {
                Log.Fatal(ex, "Application crashed during statup.");
            }
            finally
            {
                // Important: Clear buffer when app gets closed
                Log.Information("Application getting closed.");
                Log.CloseAndFlush();
            }
        }

        private static void SetupLogging()
        {
            string appDataPath = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            string logFolder = Path.Combine(appDataPath, "MicroTaskPager", "logs");
            string logFile = Path.Combine(logFolder, "log-.txt"); // "-" for date rolling

            Log.Logger = new LoggerConfiguration()
                .MinimumLevel.Information()
                .Enrich.FromLogContext()
                .WriteTo.Async(wt => wt.File(
                    path: logFile,
                    rollingInterval: RollingInterval.Day,
                    retainedFileCountLimit: 14,
                    outputTemplate: "{Timestamp:yyyy-MM-dd HH:mm:ss.fff} [{Level:u3}] ({SourceContext}) {Message:lj}{NewLine}{Exception}"))
               #if DEBUG
                .WriteTo.Debug() // For output in Visual Studio during development
               #endif
                .CreateLogger();
        }

        private static void SetupGlobalExceptionHandling()
        {
            Application.ThreadException += (sender, e) =>
            {
                Log.Fatal(e.Exception, "Catched a not handled UI-Thread-Exception.");
                MessageBox.Show("An unexpextec failure occured.", "Error", MessageBoxButtons.OK, MessageBoxIcon.Error);
            };

            // UI-Thread-Errors for Non-User-Interface-Mode
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);

            // Background-Task-Errors (.NET-global)
            TaskScheduler.UnobservedTaskException += (sender, e) =>
            {
                Log.Fatal(e.Exception, "Not handled Task-Exception in background.");
                e.SetObserved();
            };

            // General .NET AppDomain-Errors (if nothing else matched)
            AppDomain.CurrentDomain.UnhandledException += (sender, e) =>
            {
                Log.Fatal(e.ExceptionObject as Exception, "Critical AppDomain-Error.");
            };
        }
    }
}
