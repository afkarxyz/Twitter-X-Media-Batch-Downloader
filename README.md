[![GitHub All Releases](https://img.shields.io/github/downloads/afkarxyz/Twitter-X-Media-Batch-Downloader/total?style=for-the-badge)](https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases)

![TwitterXMediaBatchDownloader](https://github.com/user-attachments/assets/354d7470-c01c-4aa6-9da1-ea6c42d27330)

<div align="center">
<b>Twitter/X Media Batch Downloader</b> is a GUI tool that allows users to download all media, including images and videos, in their original quality from Twitter/X accounts, even withheld account, using <code>gallery-dl</code>
</div>

## 📥 Download

- Download the latest version of [TwitterXMediaBatchDownloader](https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases/download/v2.0/TwitterXMediaBatchDownloader.exe)
- If you're familiar with **userscripts**, please install it [here](https://greasyfork.org/en/scripts/523157-twitter-x-media-batch-downloader)

## ✨ Features

- Powered by `gallery-dl`, similar to `yt-dlp` 
- Supports downloading from **Withheld Accounts**
- **Choose media type**: All (Image + GIF + Video) or specific (Image/GIF/Video)  
- Downloads in original quality
  
## 🖼️ Screenshots

![image](https://github.com/user-attachments/assets/7c5ebdda-c558-49ed-9f16-a6060b52f6f8)

![python_7NZpAQ8RQi](https://github.com/user-attachments/assets/9982e4f5-a4b2-4d1b-9481-7bf7db700663)

![image](https://github.com/user-attachments/assets/1ef4ec73-c77b-433a-ac79-f0df5be36bd1)

![image](https://github.com/user-attachments/assets/bbc28a59-c734-4103-a7b8-05b3793f9da8)

> [!Important]
> - It is highly recommended to use the local `gallery-dl` **(by unchecking "Use API").** Use the API only for withheld accounts, as it has a 60-second timeout limit.
> - Use **Batch** if the items to be downloaded are in the thousands.

## 🔑 How to Obtain Auth Token

> [!Warning]
> - I suggest not using the **main account** to obtain the token.
> - You can use https://temp-mail.io to register a Twitter account.
> - Using an auth token or cookies has the potential to get the **account suspended.**
> - If too many media files are fetched, it will trigger a **rate limit**, and the media retrieval will fail.

1. Go to [Twitter's website](https://www.x.com/)
2. Log into your account
3. Open the Developer Tools by pressing `F12`
4. Navigate to the **Application** tab, then select **Storage** > **Cookies**
5. Find and copy the `auth_token` value or use the browser extension [X.com Auth Token Grabber](https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/releases/download/v1.0/X.com.Auth.Token.Grabber.zip)
6. Do not log out of your account, as a new `auth_token` will be generated by Twitter

> X.com Auth Token Grabber

![image](https://github.com/user-attachments/assets/4bf5f787-d34f-4259-837c-07a6432c4360)

> Developer Tools

![image](https://github.com/user-attachments/assets/8e81dd8f-f8be-4254-9cf6-cacfa97743e9)
