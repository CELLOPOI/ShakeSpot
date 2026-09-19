#include "../native/platform.h"
#include "../native/cursor.h"
#include "../native/settings.h"
#include "../native/resource.h"
#include <iostream>
#include <functional>
#include <iomanip>
using namespace shakespot;
namespace {
HWND target = nullptr;
int clicks = 0, wheels = 0, dragMoves = 0;
bool hidden = false, custom = false;
HCURSOR customCursor = nullptr;
std::ofstream report;
int passed = 0;
void check(bool value, const char* name) {
    std::cout << (value ? "PASS " : "FAIL ") << name << std::endl;
    report << (value ? "PASS " : "FAIL ") << name << '\n'; report.flush();
    if (!value) throw std::runtime_error(name);
    ++passed;
}
void pump() { MSG msg{}; while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE)) { TranslateMessage(&msg); DispatchMessageW(&msg); } }
void delay(DWORD ms) { double end = clockMs() + ms; do { pump(); Sleep(5); } while(clockMs() < end); pump(); }
bool until(const std::function<bool()>& predicate, DWORD timeout = 3000) {
    double end = clockMs() + timeout;
    do { pump(); if(predicate()) return true; Sleep(5); } while(clockMs() < end);
    return predicate();
}
LRESULT CALLBACK targetProc(HWND window, UINT message, WPARAM w, LPARAM l) {
    switch (message) {
    case WM_LBUTTONDOWN: ++clicks; SetCapture(window); return 0;
    case WM_LBUTTONUP: ReleaseCapture(); return 0;
    case WM_MOUSEMOVE: if(w & MK_LBUTTON) ++dragMoves; return 0;
    case WM_MOUSEWHEEL: ++wheels; return 0;
    case WM_SETCURSOR: SetCursor(hidden ? nullptr : custom ? customCursor : LoadCursorW(nullptr, IDC_ARROW)); return TRUE;
    case WM_PAINT: {
        PAINTSTRUCT ps{}; HDC dc = BeginPaint(window, &ps); RECT rect{}; GetClientRect(window,&rect);
        FillRect(dc,&rect,static_cast<HBRUSH>(GetStockObject(WHITE_BRUSH)));
        SetTextColor(dc,RGB(25,25,25)); SetBkMode(dc,TRANSPARENT);
        DrawTextW(dc,L"ShakeSpot native desktop verification\nOnly this test window receives generated input.",-1,&rect,DT_CENTER | DT_TOP);
        EndPaint(window,&ps); return 0;
    }
    }
    return DefWindowProcW(window,message,w,l);
}
struct Shape {
    int width=0, height=0; DWORD hotX=0, hotY=0; uint64_t hash=0;
    bool operator==(const Shape&) const = default;
};
Shape shape(HCURSOR cursor) {
    ICONINFO icon{};
    if(!cursor || !GetIconInfo(cursor,&icon)) throw std::runtime_error("GetIconInfo failed.");
    BITMAP bitmap{}; HBITMAP source=icon.hbmColor ? icon.hbmColor : icon.hbmMask;
    GetObjectW(source,sizeof(bitmap),&bitmap);
    Shape result{bitmap.bmWidth,icon.hbmColor?bitmap.bmHeight:bitmap.bmHeight/2,icon.xHotspot,icon.yHotspot,1469598103934665603ull};
    BITMAPINFO info{}; info.bmiHeader.biSize=sizeof(BITMAPINFOHEADER); info.bmiHeader.biWidth=bitmap.bmWidth;
    info.bmiHeader.biHeight=-bitmap.bmHeight; info.bmiHeader.biPlanes=1;info.bmiHeader.biBitCount=32;info.bmiHeader.biCompression=BI_RGB;
    std::vector<uint32_t> data(static_cast<size_t>(bitmap.bmWidth)*bitmap.bmHeight);
    HDC dc=CreateCompatibleDC(nullptr);
    if(!GetDIBits(dc,source,0,bitmap.bmHeight,data.data(),&info,DIB_RGB_COLORS)) throw std::runtime_error("GetDIBits failed.");
    for(auto pixel:data) {result.hash^=pixel;result.hash*=1099511628211ull;}
    DeleteDC(dc); if(icon.hbmColor)DeleteObject(icon.hbmColor);if(icon.hbmMask)DeleteObject(icon.hbmMask);
    return result;
}
Shape arrowShape() {return shape(LoadCursorW(nullptr,IDC_ARROW));}
POINT center() {RECT rect{};GetClientRect(target,&rect);POINT p{rect.right/2,rect.bottom/2};ClientToScreen(target,&p);return p;}
void moveTo(POINT point) {
    INPUT input{};input.type=INPUT_MOUSE;
    int left=GetSystemMetrics(SM_XVIRTUALSCREEN),top=GetSystemMetrics(SM_YVIRTUALSCREEN);
    int width=GetSystemMetrics(SM_CXVIRTUALSCREEN),height=GetSystemMetrics(SM_CYVIRTUALSCREEN);
    input.mi.dx=static_cast<LONG>(std::lround((point.x-left)*65535.0/(width-1)));
    input.mi.dy=static_cast<LONG>(std::lround((point.y-top)*65535.0/(height-1)));
    input.mi.dwFlags=MOUSEEVENTF_ABSOLUTE|MOUSEEVENTF_VIRTUALDESK|MOUSEEVENTF_MOVE;
    if(SendInput(1,&input,sizeof(input))!=1)throw std::runtime_error("SendInput blocked; run on an interactive desktop.");
    pump();
}
void button(DWORD flags, DWORD data=0) {
    POINT p{};GetPhysicalCursorPos(&p);
    if(WindowFromPoint(p)!=target && GetCapture()!=target)throw std::runtime_error("Input target is obscured; aborting synthetic input.");
    INPUT input{};input.type=INPUT_MOUSE;input.mi.dwFlags=flags;input.mi.mouseData=data;
    if(SendInput(1,&input,sizeof(input))!=1)throw std::runtime_error("Button injection failed.");
    delay(25);
}
void wave(DWORD duration=900, bool vertical=false) {
    POINT origin=center(); double begin=clockMs();
    do { double t=clockMs()-begin;POINT p=origin;LONG delta=static_cast<LONG>(150*std::sin(t/48));if(vertical)p.y+=delta;else p.x+=delta;moveTo(p);delay(10); } while(clockMs()-begin<duration);
}
void relativeWave(DWORD duration=800, double angle=0.0) {
    double begin=clockMs(); LONG lastX=0,lastY=0;
    do {
        double t=clockMs()-begin, amplitude=340*std::sin(t/48);
        LONG x=static_cast<LONG>(std::lround(amplitude*std::cos(angle)));
        LONG y=static_cast<LONG>(std::lround(amplitude*std::sin(angle)));
        INPUT input{};input.type=INPUT_MOUSE;input.mi.dx=x-lastX;input.mi.dy=y-lastY;
        input.mi.dwFlags=MOUSEEVENTF_MOVE|MOUSEEVENTF_MOVE_NOCOALESCE;
        if(SendInput(1,&input,sizeof(input))!=1)throw std::runtime_error("Relative input injection failed.");
        lastX=x;lastY=y;delay(5);
    } while(clockMs()-begin<duration);
}
struct CursorClip {
    RECT previous{};
    explicit CursorClip(POINT p) {
        if(!GetClipCursor(&previous))throw std::runtime_error("Cannot read cursor clip.");
        RECT rect{p.x,p.y,p.x+1,p.y+1};
        if(!ClipCursor(&rect))throw std::runtime_error("Cannot set temporary cursor clip.");
    }
    ~CursorClip(){ClipCursor(&previous);}
};
LRESULT CALLBACK peerProc(HWND window,UINT message,WPARAM w,LPARAM l) {
    if(message==WM_DESTROY){PostQuitMessage(0);return 0;}
    return DefWindowProcW(window,message,w,l);
}
int peerMain() {
    WNDCLASSW cls{};cls.hInstance=GetModuleHandleW(nullptr);cls.lpszClassName=L"ShakeSpot.AllowedPeer";
    cls.lpfnWndProc=peerProc;cls.hCursor=LoadCursorW(nullptr,IDC_ARROW);cls.hbrBackground=reinterpret_cast<HBRUSH>(COLOR_WINDOW+1);
    RegisterClassW(&cls);
    HWND window=CreateWindowExW(0,cls.lpszClassName,L"ShakeSpot allowed application test",WS_OVERLAPPEDWINDOW,
        1250,180,650,350,nullptr,nullptr,cls.hInstance,nullptr);
    if(!window)return 2;
    ShowWindow(window,SW_SHOWNOACTIVATE);ShowWindow(window,SW_SHOWNOACTIVATE);
    MSG msg{};while(GetMessageW(&msg,nullptr,0,0)>0){TranslateMessage(&msg);DispatchMessageW(&msg);}
    return 0;
}
struct AllowedPeer {
    Handle process;HWND window=nullptr;
    AllowedPeer() {
        auto path=std::filesystem::path(executablePath()).parent_path()/L"ShakeSpot.AllowedTarget.exe";
        std::filesystem::copy_file(executablePath(),path,std::filesystem::copy_options::overwrite_existing);
        auto command=quote(path.wstring())+L" --foreground-peer";
        STARTUPINFOW startup{sizeof(startup)};startup.dwFlags=STARTF_USESHOWWINDOW;startup.wShowWindow=SW_HIDE;
        PROCESS_INFORMATION child{};
        if(!CreateProcessW(nullptr,command.data(),nullptr,nullptr,FALSE,CREATE_NO_WINDOW,nullptr,nullptr,&startup,&child))
            throw std::runtime_error("Allowed peer startup failed.");
        CloseHandle(child.hThread);process.value=child.hProcess;
        if(!until([&]{window=FindWindowW(L"ShakeSpot.AllowedPeer",nullptr);return window!=nullptr;})){
            TerminateProcess(process.get(),3);throw std::runtime_error("Allowed peer window missing.");
        }
    }
    ~AllowedPeer(){
        if(window)PostMessageW(window,WM_CLOSE,0,0);
        if(WaitForSingleObject(process.get(),3000)==WAIT_TIMEOUT)TerminateProcess(process.get(),3);
    }
};

LRESULT query(HWND window, UINT field) {
    DWORD_PTR result=0;
    if(!SendMessageTimeoutW(window,QueryMessage,field,0,SMTO_ABORTIFHUNG,500,&result))throw std::runtime_error("Application query timed out.");
    return static_cast<LRESULT>(result);
}
struct Child {
    PROCESS_INFORMATION process{}; Handle guard; HWND window=nullptr;
    Child(const std::wstring& path,const std::wstring& directory) {
        Settings settings; if(!saveSettings(directory,settings))throw std::runtime_error("Test settings write failed.");
        std::wstring command=quote(path)+L" --test-mode --data-dir "+quote(directory);
        STARTUPINFOW startup{sizeof(startup)};startup.dwFlags=STARTF_USESHOWWINDOW | STARTF_FORCEOFFFEEDBACK;startup.wShowWindow=SW_HIDE;
        if(!CreateProcessW(nullptr,command.data(),nullptr,nullptr,FALSE,CREATE_NO_WINDOW,nullptr,nullptr,&startup,&process))throw std::runtime_error("Application startup failed.");
        CloseHandle(process.hThread);process.hThread=nullptr;
        if(!until([&]{window=FindWindowW(WindowClass,nullptr);return window!=nullptr;},5000))throw std::runtime_error("Application control window did not appear.");
        DWORD guardian=static_cast<DWORD>(query(window,4));
        guard.value=OpenProcess(PROCESS_QUERY_INFORMATION|PROCESS_VM_READ|SYNCHRONIZE|PROCESS_TERMINATE,FALSE,guardian);
        if(!guard.get())throw std::runtime_error("Recovery process missing.");
    }
    void command(Command command) {PostMessageW(window,CommandMessage,command,0);delay(30);}
    HWND settings() {
        command(OpenSettings);HWND dialog=nullptr;
        if(!until([&]{dialog=reinterpret_cast<HWND>(query(window,10));return dialog&&IsWindowVisible(dialog);}))
            throw std::runtime_error("Settings dialog did not open.");
        return dialog;
    }
    void preview() {moveTo(center());delay(30);if(!until([&]{SetForegroundWindow(target);return GetForegroundWindow()==target;},1500))throw std::runtime_error("External focus change prevented preview precondition.");command(Preview);check(until([&]{return query(window,1)!=0;},1200),"System cursor replacement becomes active");delay(180);}
    void stop() {if(WaitForSingleObject(process.hProcess,0)==WAIT_TIMEOUT){PostMessageW(window,CommandMessage,Quit,0);if(!until([&]{return WaitForSingleObject(process.hProcess,0)==WAIT_OBJECT_0;}))TerminateProcess(process.hProcess,4);}WaitForSingleObject(process.hProcess,3000);WaitForSingleObject(guard.get(),3000);}
    ~Child(){stop();if(process.hProcess)CloseHandle(process.hProcess);}
};
double cpu(HANDLE process) {FILETIME c{},e{},k{},u{};if(!GetProcessTimes(process,&c,&e,&k,&u))throw std::runtime_error("GetProcessTimes failed.");ULARGE_INTEGER kernel{},user{};kernel.LowPart=k.dwLowDateTime;kernel.HighPart=k.dwHighDateTime;user.LowPart=u.dwLowDateTime;user.HighPart=u.dwHighDateTime;return (kernel.QuadPart+user.QuadPart)/10000000.0;}
std::pair<double,double> memory(HANDLE process) {PROCESS_MEMORY_COUNTERS_EX counters{};counters.cb=sizeof(counters);if(!GetProcessMemoryInfo(process,reinterpret_cast<PROCESS_MEMORY_COUNTERS*>(&counters),sizeof(counters)))throw std::runtime_error("GetProcessMemoryInfo failed.");return {counters.WorkingSetSize/1048576.0,counters.PrivateUsage/1048576.0};}
void measure(Child& child,const char* name,int mode) {
    double begin=clockMs(),start=cpu(child.process.hProcess)+cpu(child.guard.get()),nextPreview=0,maxWorking=0,maxPrivate=0;
    auto startEvents=query(child.window,9),startSamples=query(child.window,5),startUpdates=query(child.window,2),startTicks=query(child.window,8);
    POINT origin=center();
    while(clockMs()-begin<10000) {
        double t=clockMs()-begin;
        if(mode==1 && t>=nextPreview){PostMessageW(child.window,CommandMessage,Preview,0);nextPreview=t+450;}
        if(mode==2){POINT p=origin;p.x+=static_cast<LONG>(150*std::sin(t/48));moveTo(p);}
        auto main=memory(child.process.hProcess),guard=memory(child.guard.get());
        maxWorking=std::max(maxWorking,main.first+guard.first);maxPrivate=std::max(maxPrivate,main.second+guard.second);
        delay(mode==2?10:25);
    }
    double elapsed=(clockMs()-begin)/1000,total=cpu(child.process.hProcess)+cpu(child.guard.get())-start;
    report<<std::fixed<<std::setprecision(3)<<"PERF "<<name<<" wall_s="<<elapsed<<" cpu_s="<<total<<" one_core_percent="<<100*total/elapsed<<" combined_working_MiB="<<maxWorking<<" combined_private_MiB="<<maxPrivate
        <<" raw_events="<<query(child.window,9)-startEvents<<" samples="<<query(child.window,5)-startSamples<<" cursor_updates="<<query(child.window,2)-startUpdates<<" animation_ticks="<<query(child.window,8)-startTicks<<'\n';report.flush();
    std::cout<<"MEASURED "<<name<<std::endl;
}
void snapshotCursor(const std::wstring& file) {
    CURSORINFO cursor{};if(!cursorInfo(cursor))throw std::runtime_error("Cursor query failed.");auto geometry=shape(cursor.hCursor);
    constexpr int size=320;BITMAPINFO info{};info.bmiHeader.biSize=sizeof(BITMAPINFOHEADER);info.bmiHeader.biWidth=size;info.bmiHeader.biHeight=-size;info.bmiHeader.biPlanes=1;info.bmiHeader.biBitCount=32;
    void* pixels=nullptr;HBITMAP bitmap=CreateDIBSection(nullptr,&info,DIB_RGB_COLORS,&pixels,nullptr,0);HDC dc=CreateCompatibleDC(nullptr);HGDIOBJ old=SelectObject(dc,bitmap);
    std::fill_n(static_cast<uint32_t*>(pixels),size*size,0xffd8e6f0);DrawIconEx(dc,20,20,cursor.hCursor,geometry.width,geometry.height,0,nullptr,DI_NORMAL);GdiFlush();
    BITMAPFILEHEADER header{};header.bfType=0x4d42;header.bfOffBits=sizeof(header)+sizeof(BITMAPINFOHEADER);header.bfSize=header.bfOffBits+size*size*4;
    std::ofstream out(std::filesystem::path(file),std::ios::binary);out.write(reinterpret_cast<char*>(&header),sizeof(header));out.write(reinterpret_cast<char*>(&info.bmiHeader),sizeof(info.bmiHeader));out.write(static_cast<char*>(pixels),size*size*4);
    SelectObject(dc,old);DeleteDC(dc);DeleteObject(bitmap);
}
void snapshotWindow(HWND window, const std::wstring& file) {
    RECT rect{};GetWindowRect(window,&rect);int width=rect.right-rect.left,height=rect.bottom-rect.top;
    BITMAPINFO info{};info.bmiHeader.biSize=sizeof(BITMAPINFOHEADER);info.bmiHeader.biWidth=width;info.bmiHeader.biHeight=-height;info.bmiHeader.biPlanes=1;info.bmiHeader.biBitCount=32;
    void* pixels=nullptr;HBITMAP bitmap=CreateDIBSection(nullptr,&info,DIB_RGB_COLORS,&pixels,nullptr,0);HDC dc=CreateCompatibleDC(nullptr);HGDIOBJ old=SelectObject(dc,bitmap);
    if(!PrintWindow(window,dc,0))throw std::runtime_error("Settings capture failed.");GdiFlush();
    BITMAPFILEHEADER header{};header.bfType=0x4d42;header.bfOffBits=sizeof(header)+sizeof(BITMAPINFOHEADER);header.bfSize=header.bfOffBits+width*height*4;
    std::ofstream out(std::filesystem::path(file),std::ios::binary);out.write(reinterpret_cast<char*>(&header),sizeof(header));out.write(reinterpret_cast<char*>(&info.bmiHeader),sizeof(info.bmiHeader));out.write(static_cast<char*>(pixels),static_cast<std::streamsize>(width)*height*4);
    SelectObject(dc,old);DeleteDC(dc);DeleteObject(bitmap);
}
struct DesktopCleanup {
    POINT position{};HWND foreground=GetForegroundWindow();bool changed=false;
    DesktopCleanup(){GetPhysicalCursorPos(&position);}
    ~DesktopCleanup(){if(changed)reloadCursors();if(GetCapture()==target)ReleaseCapture();if(target)DestroyWindow(target);if(customCursor)DestroyCursor(customCursor);SetPhysicalCursorPos(position.x,position.y);if(foreground)SetForegroundWindow(foreground);}
};
}
int wmain(int argc,wchar_t** argv) {
    if(argc>1&&std::wstring(argv[1])==L"--foreground-peer")return peerMain();
    DesktopCleanup cleanup;
    try {
        if(FindWindowW(WindowClass,nullptr))throw std::runtime_error("Close the existing native application before running desktop tests.");
        bool measureOnly=argc>2&&std::wstring(argv[2])==L"--measure-only";
        auto root=std::filesystem::path(executablePath()).parent_path();
        auto directory=root/(measureOnly?L"performance-checks":L"desktop-checks");
        std::filesystem::create_directories(directory);report.open(directory/L"report.txt");
        std::wstring path=argc>1?argv[1]:(root/L"ShakeSpot.Native.exe").wstring();
        WNDCLASSW type{};type.hInstance=GetModuleHandleW(nullptr);type.lpszClassName=L"ShakeSpot.Native.TestTarget";type.lpfnWndProc=targetProc;type.hCursor=LoadCursorW(nullptr,IDC_ARROW);RegisterClassW(&type);
        POINT p{};GetPhysicalCursorPos(&p);MONITORINFO screen{sizeof(screen)};GetMonitorInfoW(MonitorFromPoint(p,MONITOR_DEFAULTTONEAREST),&screen);
        target=CreateWindowExW(WS_EX_TOPMOST,type.lpszClassName,L"ShakeSpot native desktop checks",WS_OVERLAPPEDWINDOW,screen.rcWork.left+200,screen.rcWork.top+150,1000,700,nullptr,nullptr,type.hInstance,nullptr);
        ShowWindow(target,SW_SHOW);ShowWindow(target,SW_RESTORE);SetForegroundWindow(target);UpdateWindow(target);delay(300);moveTo(center());delay(50);
        until([&]{SetForegroundWindow(target);return GetForegroundWindow()==target && WindowFromPoint(center())==target;},3000);
        wchar_t foregroundTitle[256]{},atPointTitle[256]{}; GetWindowTextW(GetForegroundWindow(),foregroundTitle,256); GetWindowTextW(WindowFromPoint(center()),atPointTitle,256);
        if(GetForegroundWindow()!=target || WindowFromPoint(center())!=target) std::wcerr<<L"Foreground: "<<foregroundTitle<<L"; at test center: "<<atPointTitle<<std::endl;
        check(GetForegroundWindow()==target && WindowFromPoint(center())==target,"Owned input target is foreground and unobscured");
        Shape baseline=arrowShape();report<<"BASELINE height="<<baseline.height<<" width="<<baseline.width<<" dpi="<<GetDpiForWindow(target)<<'\n';
        cleanup.changed=true;
        if(measureOnly) {
            Child child(path,(directory/L"profile").wstring());
            delay(1000);
            measure(child,"effect_held_static",1);delay(1500);
            measure(child,"continuous_shake",2);
            child.command(Pause);
            check(!query(child.window,1)&&arrowShape()==baseline,"Performance run restores cursor");
            return 0;
        }
        {
            Child child(path,(directory/L"profile").wstring());
            check(query(child.window,6)!=0,"Tray application and recovery process start enabled");
            delay(150);LRESULT sampleStart=query(child.window,5);delay(500);
            check(query(child.window,5)==sampleStart,"Idle detection has no position polling");
            measure(child,"idle_before_settings",0);
            child.preview();
            CURSORINFO current{};
            check(cursorInfo(current),"Current visible cursor can be queried");
            auto enlarged=shape(current.hCursor);
            report<<"ACTIVE height="<<enlarged.height<<" width="<<enlarged.width<<" hotX="<<enlarged.hotX<<" hotY="<<enlarged.hotY<<'\n';
            CURSORINFO diagnostic{};cursorInfo(diagnostic);auto currentShape=shape(diagnostic.hCursor);
            report<<"STATE dirty="<<query(child.window,1)<<" updates="<<query(child.window,2)<<" skipped="<<query(child.window,7)<<" ticks="<<query(child.window,8)<<" reason="<<query(child.window,11)<<" role="<<query(child.window,12)<<" current_height="<<currentShape.height<<'\n';
            check(enlarged.height>baseline.height && enlarged.height<=256,"Real system arrow bitmap is enlarged within limit");
            CursorRoles knownRoles;check(knownRoles.identify(current.hCursor)>=0,"Visible cursor is a replaced standard system cursor");
            check(enlarged.hotX==2&&enlarged.hotY==2,"Normal arrow hotspot aligns with its visible tip");
            check(GetForegroundWindow()==target,"Activation preserves foreground focus");
            snapshotCursor((directory/L"actual-system-cursor.bmp").wstring());
            int wheelBefore=wheels;button(MOUSEEVENTF_WHEEL,WHEEL_DELTA);
            check(wheels==wheelBefore+1,"Wheel reaches foreground application during effect");
            int clickBefore=clicks;button(MOUSEEVENTF_LEFTDOWN);button(MOUSEEVENTF_LEFTUP);
            check(clicks==clickBefore+1,"Click reaches foreground application during effect");
            check(until([&]{return !query(child.window,1);}),"Button press immediately restores cursor");
            child.preview();button(MOUSEEVENTF_LEFTDOWN);POINT start=center();for(int i=1;i<=8;++i){moveTo({start.x+i*8,start.y});delay(15);}button(MOUSEEVENTF_LEFTUP);
            check(dragMoves>=5&&GetForegroundWindow()==target,"Drag and focus remain functional");
            delay(220);auto triggers=query(child.window,3);wave();
            check(query(child.window,9)>0&&query(child.window,5)>sampleStart,"Raw Input receives motion and samples movement");
            check(query(child.window,3)>triggers,"Horizontal generated shake triggers real application");
            check(until([&]{return !query(child.window,1);},2000)&&arrowShape()==baseline,"Animation automatically restores configured cursor pixels and hotspot");
            triggers=query(child.window,3);wave(950,true);check(query(child.window,3)>triggers,"Vertical generated shake triggers real application");
            child.command(Pause);check(!query(child.window,1)&&arrowShape()==baseline,"Pause restores cursor immediately");
            triggers=query(child.window,3);wave();check(query(child.window,3)==triggers,"Paused application ignores shakes");
            child.command(Resume);wave();check(query(child.window,3)>triggers,"Resume accepts fresh shake");
            child.command(Pause);child.command(Resume);
            hidden=true;moveTo(center());delay(50);SendMessageW(target,WM_SETCURSOR,reinterpret_cast<WPARAM>(target),MAKELPARAM(HTCLIENT,WM_MOUSEMOVE));
            child.command(Preview);check(!query(child.window,1),"Application-hidden cursor suppresses effect");
            hidden=false;moveTo({center().x+1,center().y});delay(50);
            customCursor=makeArrow(40);custom=true;moveTo(center());delay(50);
            child.command(Preview);check(!query(child.window,1),"Custom application cursor is left unchanged");
            custom=false;moveTo({center().x+1,center().y});delay(50);
            RECT originalBounds{};GetWindowRect(target,&originalBounds);
            LONG_PTR originalStyle=GetWindowLongPtrW(target,GWL_STYLE);
            SetWindowLongPtrW(target,GWL_STYLE,WS_POPUP);
            SetWindowPos(target,HWND_TOPMOST,screen.rcMonitor.right-1000,screen.rcMonitor.bottom-700,1000,700,SWP_FRAMECHANGED|SWP_SHOWWINDOW);
            POINT corner{screen.rcMonitor.right-4,screen.rcMonitor.bottom-4};moveTo(corner);delay(100);
            check(WindowFromPoint(corner)==target,"Owned test window covers physical bottom-right corner");
            child.command(Preview);delay(200);CURSORINFO edgeCursor{};check(cursorInfo(edgeCursor),"Edge cursor can be queried");auto edge=shape(edgeCursor.hCursor);
            check(edge.hotX==static_cast<DWORD>(edge.width-3)&&edge.hotY==static_cast<DWORD>(edge.height-3),"Screen edge mirrors actual cursor bitmap and hotspot");
            POINT actualPoint{};GetPhysicalCursorPos(&actualPoint);check(std::abs(actualPoint.x-corner.x)<=1&&std::abs(actualPoint.y-corner.y)<=1,"Edge effect preserves actual click position");
            child.command(Pause);child.command(Resume);
            SetWindowLongPtrW(target,GWL_STYLE,originalStyle);
            SetWindowPos(target,HWND_TOPMOST,originalBounds.left,originalBounds.top,originalBounds.right-originalBounds.left,originalBounds.bottom-originalBounds.top,SWP_FRAMECHANGED|SWP_SHOWWINDOW);
            moveTo(center());delay(100);
            HWND dialog=child.settings();check(dialog&&IsWindowVisible(dialog),"Native settings dialog opens");
            snapshotWindow(dialog,(directory/L"settings.bmp").wstring());
            check(IsDlgButtonChecked(dialog,IDC_STARTUP)==BST_UNCHECKED,"Startup defaults to disabled on this test machine");
            SendDlgItemMessageW(dialog,IDC_SENSITIVITY,CB_SETCURSEL,3,0);SendDlgItemMessageW(dialog,IDC_SCALE,CB_SETCURSEL,6,0);SendDlgItemMessageW(dialog,IDC_DURATION,CB_SETCURSEL,3,0);
            SendMessageW(dialog,WM_COMMAND,IDOK,0);delay(100);auto saved=loadSettings((directory/L"profile").wstring());
            check(saved.sensitivity==4&&saved.maximumScale==5&&saved.durationMs==1500,"Settings save and reload native configuration");
            dialog=child.settings();SendMessageW(dialog,WM_COMMAND,IDC_DEFAULTS,0);SendMessageW(dialog,WM_COMMAND,IDOK,0);delay(100);
            saved=loadSettings((directory/L"profile").wstring());check(saved.sensitivity==3&&saved.maximumScale==4&&saved.durationMs==1100,"Defaults restore standard settings");
            child.command(Pause);child.command(Resume);moveTo(center());delay(220);
            triggers=query(child.window,3);relativeWave(800,0.7853981633974483);
            check(query(child.window,3)>triggers,"Relative diagonal shake triggers real application");
            child.command(Pause);child.command(Resume);moveTo(center());delay(220);
            {
                POINT held=center();CursorClip clip(held);triggers=query(child.window,3);
                relativeWave();POINT after{};GetPhysicalCursorPos(&after);
                check(after.x==held.x&&after.y==held.y,"Temporary clip keeps screen coordinates stationary");
                check(query(child.window,3)>triggers,"Raw relative movement triggers despite stationary clipped cursor");
            }
            child.command(Pause);child.command(Resume);moveTo(center());delay(100);
            {
                AllowedPeer peer;
                auto excludedName=std::filesystem::path(executablePath()).filename().wstring();
                dialog=child.settings();
                // 跨进程必须显式发送文本消息，不能只改到 USER32 的窗口文本缓存。
                check(SendDlgItemMessageW(dialog,1007,WM_SETTEXT,0,reinterpret_cast<LPARAM>(L"*.exe"))!=0,"Exclusion edit accepts text");
                wchar_t invalidText[512]{};SendDlgItemMessageW(dialog,1007,WM_GETTEXT,512,reinterpret_cast<LPARAM>(invalidText));
                check(std::wstring(invalidText)==L"*.exe","Exclusion text reaches the real edit control");
                SendMessageW(dialog,WM_COMMAND,IDOK,0);delay(80);
                check(query(child.window,10)!=0,"Invalid exclusion keeps settings open");
                snapshotWindow(dialog,(directory/L"settings-invalid-rule.bmp").wstring());
                SendDlgItemMessageW(dialog,1007,WM_SETTEXT,0,reinterpret_cast<LPARAM>(excludedName.c_str()));
                SendMessageW(dialog,WM_COMMAND,IDOK,0);delay(100);
                SetForegroundWindow(target);
                check(until([&]{return query(child.window,0)==1;}),"Excluded foreground app temporarily suppresses effects");
                check(query(child.window,6)==1,"Application exclusion preserves user's enabled setting");
                triggers=query(child.window,3);wave(650);
                check(query(child.window,3)==triggers&&!query(child.window,1),"Excluded app receives motion without activation");
                check(until([&]{SetForegroundWindow(peer.window);return GetForegroundWindow()==peer.window&&query(child.window,0)==0;}),"Allowed foreground app resumes detection");
                delay(100);wave(120);
                check(query(child.window,3)==triggers,"Leaving excluded app does not reuse old gesture evidence");
                wave(750);check(query(child.window,3)>triggers,"Fresh shake activates in allowed app");
                SetForegroundWindow(target);
                check(until([&]{return query(child.window,0)==1&&!query(child.window,1);})&&arrowShape()==baseline,"Entering excluded app restores active system cursor");
                child.command(Pause);
                check(until([&]{SetForegroundWindow(peer.window);return query(child.window,0)==0;}),"Foreground notifications work while manually paused");
                wave(650);check(query(child.window,6)==0&&!query(child.window,1),"Application switch cannot undo manual pause");
                child.command(Resume);
                dialog=child.settings();
                wchar_t exclusionText[512]{};SendDlgItemMessageW(dialog,1007,WM_GETTEXT,512,reinterpret_cast<LPARAM>(exclusionText));
                check(_wcsicmp(exclusionText,excludedName.c_str())==0,"Settings retain the saved exclusion rule");
                snapshotWindow(dialog,(directory/L"settings-with-exclusions.bmp").wstring());
                SendDlgItemMessageW(dialog,1007,WM_SETTEXT,0,reinterpret_cast<LPARAM>(L""));SendMessageW(dialog,WM_COMMAND,IDOK,0);delay(100);
                SetForegroundWindow(target);
                check(until([&]{return !query(child.window,0)&&query(child.window,6);}),"Removing exclusion restores normal detection");
            }
            SetForegroundWindow(target);moveTo(center());delay(3500);
            measure(child,"idle_after_settings",0);
            measure(child,"effect_held_static",1);delay(1500);
            measure(child,"continuous_shake",2);delay(1500);
            child.command(Pause);measure(child,"paused",0);child.command(Resume);
            delay(3500);
            DWORD gdiBefore=GetGuiResources(child.process.hProcess,GR_GDIOBJECTS),userBefore=GetGuiResources(child.process.hProcess,GR_USEROBJECTS),handlesBefore=0;
            GetProcessHandleCount(child.process.hProcess,&handlesBefore);
            for(int cycle=0;cycle<16;++cycle){
                child.command(Preview);delay(170);
                check(query(child.window,13)<=16 && query(child.window,14)<=2*1024*1024,"Cursor cache remains within entry and byte budgets");
                child.command(Pause);child.command(Resume);
            }
            delay(3500);
            DWORD handlesAfter=0;GetProcessHandleCount(child.process.hProcess,&handlesAfter);
            check(query(child.window,13)==0 && query(child.window,14)==0,"Inactive cursor cache releases all frames and accounted bytes");
            check(GetGuiResources(child.process.hProcess,GR_GDIOBJECTS)==gdiBefore && GetGuiResources(child.process.hProcess,GR_USEROBJECTS)==userBefore && handlesBefore==handlesAfter,"Repeated effects return GDI USER and kernel handles to baseline");
            report<<"RESOURCES gdi_before="<<gdiBefore<<" gdi_after="<<GetGuiResources(child.process.hProcess,GR_GDIOBJECTS)<<" user_before="<<userBefore<<" user_after="<<GetGuiResources(child.process.hProcess,GR_USEROBJECTS)<<" handles_before="<<handlesBefore<<" handles_after="<<handlesAfter<<'\n';
            moveTo(center());delay(100);
            auto burstEvents=query(child.window,9),burstSamples=query(child.window,5);
            INPUT burst[128]{};
            for(int i=0;i<128;++i){burst[i].type=INPUT_MOUSE;burst[i].mi.dx=i%2?1:-1;burst[i].mi.dwFlags=MOUSEEVENTF_MOVE|MOUSEEVENTF_MOVE_NOCOALESCE;}
            double burstStart=clockMs();
            for(int batch=0;batch<64;++batch){
                if(WindowFromPoint(center())!=target)throw std::runtime_error("Burst input target obscured.");
                if(SendInput(128,burst,sizeof(INPUT))!=128)throw std::runtime_error("Burst input injection failed.");
                delay(1);
            }
            check(until([&]{return query(child.window,9)-burstEvents>=8192;}),"Raw Input processes 8192 generated burst events");
            double burstElapsed=clockMs()-burstStart;
            auto sampled=query(child.window,5)-burstSamples;
            check(sampled<=static_cast<LRESULT>(std::ceil(burstElapsed/8))+3 && query(child.window,6),"Burst sampling remains time-bounded and application stays enabled");
            report<<"BURST events="<<query(child.window,9)-burstEvents<<" samples="<<sampled<<" elapsed_ms="<<burstElapsed<<'\n';
            PostMessageW(child.window,WM_APP+1,0,WM_CONTEXTMENU);
            check(until([&]{GUITHREADINFO info{sizeof(info)};return GetGUIThreadInfo(GetWindowThreadProcessId(child.window,nullptr),&info)&&(info.flags&GUI_INMENUMODE);}),"Tray context menu enters native menu loop");
            wave(450);
            PostMessageW(child.window,WM_CANCELMODE,0,0);delay(200);
            check(query(child.window,6)!=0,"Closing tray menu after motion keeps effects enabled");
            SetForegroundWindow(target);moveTo(center());delay(200);
            child.preview();child.command(Quit);check(until([&]{return WaitForSingleObject(child.process.hProcess,0)==WAIT_OBJECT_0;}),"Normal exit terminates main process");
            check(until([&]{return WaitForSingleObject(child.guard.get(),0)==WAIT_OBJECT_0;}),"Normal exit terminates recovery process");
            check(arrowShape()==baseline,"Normal exit restores original cursor");
        }
        {
            Child child(path,(directory/L"crash-profile").wstring());child.preview();TerminateProcess(child.process.hProcess,9);
            check(until([&]{return WaitForSingleObject(child.guard.get(),0)==WAIT_OBJECT_0;})&&arrowShape()==baseline,"Recovery process restores cursor after forced main termination");
            int before=clicks;button(MOUSEEVENTF_LEFTDOWN);button(MOUSEEVENTF_LEFTUP);check(clicks==before+1,"Input remains usable after crash recovery");
        }
        {
            Child child(path,(directory/L"hang-profile").wstring());child.preview();PostMessageW(child.window,CommandMessage,TestHang,0);
            check(until([&]{return WaitForSingleObject(child.process.hProcess,0)==WAIT_OBJECT_0;},5000),"Recovery process terminates own hung parent");
            check(until([&]{return WaitForSingleObject(child.guard.get(),0)==WAIT_OBJECT_0;})&&arrowShape()==baseline,"Hung animation recovery restores cursor");
        }
        {
            Child child(path,(directory/L"guard-profile").wstring());child.preview();TerminateProcess(child.guard.get(),9);
            check(until([&]{return !query(child.window,6);})&&!query(child.window,1)&&arrowShape()==baseline,"Guardian failure makes main process restore and pause");
        }
        {
            auto recoveryDirectory=(directory/L"restart-profile").wstring();
            {Child child(path,recoveryDirectory);child.preview();
                // 同时挂起并终止两个自建进程，模拟任务管理器结束整个进程树。
                using NtSuspendProcess=LONG(NTAPI*)(HANDLE);
                auto suspend=reinterpret_cast<NtSuspendProcess>(GetProcAddress(GetModuleHandleW(L"ntdll.dll"),"NtSuspendProcess"));
                if(!suspend)throw std::runtime_error("Test suspend API unavailable.");
                suspend(child.process.hProcess);suspend(child.guard.get());TerminateProcess(child.process.hProcess,9);TerminateProcess(child.guard.get(),9);
                WaitForSingleObject(child.process.hProcess,2000);WaitForSingleObject(child.guard.get(),2000);
                check(std::filesystem::exists(std::filesystem::path(recoveryDirectory)/L"native-recovery.flag"),"Forced process-tree termination preserves recovery marker");
            }
            {Child child(path,recoveryDirectory);check(arrowShape()==baseline&&!std::filesystem::exists(std::filesystem::path(recoveryDirectory)/L"native-recovery.flag"),"Next startup recovers stale system cursor state");}
        }
        CURSORINFO finalCursor{};
        check(cursorInfo(finalCursor)&& (finalCursor.flags&CURSOR_SHOWING),"Cursor remains visible after all recovery scenarios");
        report<<"TOTAL "<<passed<<" checks passed\n";std::cout<<"TOTAL "<<passed<<" checks passed"<<std::endl;
        return 0;
    } catch(const std::exception& e) {std::cerr<<"ERROR "<<e.what()<<std::endl;if(report)report<<"ERROR "<<e.what()<<'\n';return 1;}
}



