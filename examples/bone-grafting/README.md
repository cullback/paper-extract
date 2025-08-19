# Paper Extract

A tool for extracting data from scientific papers using AI.

## Quick Start (macOS)

### 1. Download the App

1. Go to the [Releases page](https://github.com/cullback/paper-extract/releases)
2. Find the latest version
3. Download `paper-extract-aarch64-darwin.tar.gz` (for Apple Silicon Macs)
4. Double-click the downloaded file to extract it
5. You should now have a file called `paper-extract-aarch64-darwin`

### 2. Set Up Your Working Folder

1. Create a new folder on your Desktop called "paper-extraction"
2. Put all the PDF files you want to process inside this folder
3. Move the `paper-extract-aarch64-darwin` file into this same folder
4. Create or put your `schema.csv` file in this folder too

### 3. Set Up Your API Key

1. Get an API key from [OpenRouter](https://openrouter.ai)
2. Open Terminal (search "Terminal" in Spotlight or find it in Applications > Utilities)
3. Type this command and press Enter (replace `your-api-key-here` with your actual API key):
   ```
   export OPENROUTER_API_KEY=your-api-key-here
   ```

### 4. Run the Tool

1. In Terminal, navigate to your working folder:

   ```
   cd ~/Desktop/paper-extraction
   ```

2. Make the tool executable:

   ```
   chmod +x paper-extract-aarch64-darwin
   ```

3. Run the tool on your files:

   ```
   ./paper-extract-aarch64-darwin schema.csv document.pdf
   ```

Replace `document.pdf` with the name of your PDF file. The tool will create a CSV file with the extracted data in the same folder.

## What You Need

- A CSV file describing what data to extract (the "schema")
- A PDF file to extract data from
- An OpenRouter API key

The tool will process your PDF and create a new CSV file with the extracted data.
