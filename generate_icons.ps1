Add-Type -AssemblyName System.Drawing

$srcImage = "C:\Users\ahmad\.gemini\antigravity-ide\brain\f5a60688-bc38-45ba-9a4b-ce71ad0c80d1\hopper_app_icon_1791962619828.jpg"
$extensionIconsDir = "C:\Users\ahmad\AndroidStudioProjects\IDM_project\extension\icons"
$tauriIconsDir = "C:\Users\ahmad\AndroidStudioProjects\IDM_project\src-tauri\icons"

New-Item -ItemType Directory -Force -Path $extensionIconsDir | Out-Null
New-Item -ItemType Directory -Force -Path $tauriIconsDir | Out-Null

$img = [System.Drawing.Image]::FromFile($srcImage)

function Save-Resized-Png($image, $targetPath, $size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($image, 0, 0, $size, $size)
    $g.Dispose()
    $bmp.Save($targetPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}

# Chrome extension icons
Save-Resized-Png $img "$extensionIconsDir\icon-16.png" 16
Save-Resized-Png $img "$extensionIconsDir\icon-32.png" 32
Save-Resized-Png $img "$extensionIconsDir\icon-48.png" 48
Save-Resized-Png $img "$extensionIconsDir\icon-128.png" 128

# Tauri desktop icons
Save-Resized-Png $img "$tauriIconsDir\32x32.png" 32
Save-Resized-Png $img "$tauriIconsDir\128x128.png" 128
Save-Resized-Png $img "$tauriIconsDir\icon.png" 256

# Windows ICO file from 32x32 bitmap
$bmp32 = New-Object System.Drawing.Bitmap(32, 32)
$g32 = [System.Drawing.Graphics]::FromImage($bmp32)
$g32.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g32.DrawImage($img, 0, 0, 32, 32)
$hIcon = $bmp32.GetHicon()
$icon = [System.Drawing.Icon]::FromHandle($hIcon)
$fs = New-Object System.IO.FileStream("$tauriIconsDir\icon.ico", [System.IO.FileMode]::Create)
$icon.Save($fs)
$fs.Close()
$g32.Dispose()
$bmp32.Dispose()

$img.Dispose()
Write-Host "All icons generated successfully!"
