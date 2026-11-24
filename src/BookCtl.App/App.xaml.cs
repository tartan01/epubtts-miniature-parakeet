using BookCtl.Core;
using Microsoft.UI.Xaml;

namespace BookCtl.App;

public partial class App : Application
{
    public static BookCtlHandlers Handlers { get; private set; } = null!;
    public static MainWindow? Window { get; private set; }

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        var cfg = BookCtlConfig.Load();
        Handlers = new BookCtlHandlers(cfg);
        Handlers.Worker.Start();          // 桌面形态：内嵌常驻 worker，一键即用
        Window = new MainWindow();
        Window.Activate();
    }
}
