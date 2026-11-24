using BookCtl.Core;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Windows.Graphics;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace BookCtl.App;

public sealed partial class MainWindow : Window
{
    private readonly MainViewModel _vm;
    private readonly DispatcherQueue _dispatcher;
    private readonly DispatcherTimer _timer;

    public MainWindow()
    {
        InitializeComponent();
        Title = "本地朗读工厂";
        _dispatcher = DispatcherQueue.GetForCurrentThread();
        _vm = new MainViewModel();
        // WinUI 3 的 Window 没有 DataContext（WPF 才有），在根 Grid 上设置并供子元素继承
        Root.DataContext = _vm;

        App.Handlers.Bus.OnEvent += OnBusEvent;
        ApplyCommandLine();
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _timer.Tick += (_, _) => _vm.Refresh();
        _timer.Start();
        _vm.Refresh();
        Resize();
        ApplyOnTop();
    }

    /// <summary>--on-top：演示 / 录屏时把窗口置顶，避免被其它窗口遮挡。</summary>
    private void ApplyOnTop()
    {
        try
        {
            if (!Environment.GetCommandLineArgs().Skip(1)
                    .Any(a => a.Equals("--on-top", StringComparison.OrdinalIgnoreCase))) return;
            if (AppWindow.Presenter is OverlappedPresenter p) p.IsAlwaysOnTop = true;
        }
        catch { }
    }

    private void OnBusEvent(JobEvent e)
    {
        // 事件来自后台 worker，必须切回 UI 线程
        _dispatcher.TryEnqueue(() => _vm.AddLog(e));
    }

    /// <summary>
    /// 支持命令行：安装包注册了 .epub 关联，双击书应直接带进界面；
    /// --detailed 启动即进详细参数模式。
    /// </summary>
    private void ApplyCommandLine()
    {
        try
        {
            var args = Environment.GetCommandLineArgs().Skip(1).ToArray();
            if (args.Any(a => a.Equals("--detailed", StringComparison.OrdinalIgnoreCase)))
                _vm.IsSimpleMode = false;
            var epub = args.FirstOrDefault(a => a.EndsWith(".epub", StringComparison.OrdinalIgnoreCase) && File.Exists(a));
            if (epub is not null) _vm.InputPath = Path.GetFullPath(epub);

            // --auto-start：命令行 / 文件关联直接开始生成，满足"导入 epub 即生成朗读音频"。
            // 通过 Dispatcher 排队，确保窗口与事件订阅就绪后才启动 worker，避免事件早于 UI。
            if (args.Any(a => a.Equals("--auto-start", StringComparison.OrdinalIgnoreCase))
                && !string.IsNullOrWhiteSpace(_vm.InputPath))
            {
                _dispatcher.TryEnqueue(() =>
                {
                    if (_vm.IsSimpleMode) _vm.StartOneClick();
                    else _vm.Start();
                });
            }
        }
        catch { }
    }

    private async void Pick_Click(object sender, RoutedEventArgs e)
    {
        var picker = new FileOpenPicker
        {
            SuggestedStartLocation = PickerLocationId.DocumentsLibrary,
            ViewMode = PickerViewMode.List,
        };
        picker.FileTypeFilter.Add(".epub");
        InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(this));
        var file = await picker.PickSingleFileAsync();
        if (file is not null) _vm.InputPath = file.Path;
    }

    private void Start_Click(object sender, RoutedEventArgs e) => _vm.Start();
    private void StartSimple_Click(object sender, RoutedEventArgs e) => _vm.StartOneClick();
    private void StartDetailed_Click(object sender, RoutedEventArgs e) => _vm.Start();
    private void Cancel_Click(object sender, RoutedEventArgs e) => _vm.Cancel();
    private void OpenOutput_Click(object sender, RoutedEventArgs e) => _vm.OpenOutput();
    private void ClearLogs_Click(object sender, RoutedEventArgs e) => _vm.ClearLogs();

    private void Resize()
    {
        var hwnd = WindowNative.GetWindowHandle(this);
        var id = Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);
        AppWindow.GetFromWindowId(id)?.Resize(new SizeInt32(1400, 960));
    }
}
