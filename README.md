# rustai
Overview
TouristXi9d is a Rust-based desktop application that serves as an AI client for interacting with the Ollama API. It features Retrieval-Augmented Generation (RAG) for context-aware responses and analytics to track usage metrics. The application uses the egui library for a modern graphical user interface, allowing users to input prompts, load text files for context, view AI-generated responses, and monitor analytics like request counts and response times. Conversations are saved in a SQLite database and as text files for persistence.
Features

Send prompts to an Ollama API server and display responses.
Load text files to provide additional context for AI prompts.
RAG system to retrieve similar past conversations for improved responses.
Analytics dashboard showing total requests, average response time, and more.
Save responses to text files and open the save directory.
Modern, dark-themed UI with a clean and intuitive layout.

Prerequisites
To run the project, ensure you have the following installed:

Rust (latest stable version recommended).
Cargo (comes with Rust).
An Ollama server running locally or accessible at http://localhost:11434/api/generate.
A compatible operating system (Windows, macOS, or Linux).

Getting Started
1. Clone the Repository
Clone the project to your local machine:
git clone <repository-url>
cd tourist_xi9d

Replace <repository-url> with the actual repository URL if applicable.
2. Install Dependencies
The project uses several Rust crates (eframe, reqwest, serde, chrono, rusqlite, tokio, rfd). Install them by running:
cargo build --release

This command fetches all dependencies listed in Cargo.toml and builds the project in release mode.
3. Set Up Ollama
Ensure an Ollama server is running and accessible. By default, the application connects to http://localhost:11434/api/generate. To start an Ollama server:

Install Ollama following the instructions at https://ollama.ai/.
Run the server with:

ollama serve


Ensure the model deepseek-r1:7b (or another compatible model) is available. Pull it if needed:

ollama pull deepseek-r1:7b

4. Run the Application
Compile and run the project in release mode:
cargo run --release

This will launch the GUI application, displaying the TouristXi9d interface.
5. Using the Application

Model and URL: Configure the AI model (default: deepseek-r1:7b) and Ollama URL (default: http://localhost:11434/api/generate) in the configuration section.
Load File: Click "Load Text File" to upload a text file for additional context.
Input Prompt: Enter your prompt in the input text area and click "Generate Response" to get an AI response.
Analytics: View usage metrics in the right panel (toggle with "Show Analytics").
RAG: Enable RAG to see similar past prompts and responses (toggle with "Enable RAG").
Save and Copy: Copy or save the output to a file, or open the save directory (./tourist_data) to view saved responses.

6. Project Structure

src/main.rs: Contains the main application logic, UI, and RAG system implementation.
tourist_data/: Directory where conversation data is saved as text files and a SQLite database (conversations.db).
Cargo.toml: Lists project dependencies and configuration.

7. Troubleshooting

Ollama Connection Issues: Ensure the Ollama server is running and the URL is correct. Check firewall settings if connecting to a remote server.
Build Errors: Verify Rust is up to date (rustup update) and all dependencies are installed.
UI Issues: Ensure your system supports OpenGL (required by eframe).

8. Contributing
Feel free to submit issues or pull requests to improve the project. Ensure any changes maintain the existing functionality and focus on enhancing the UI or performance without adding new features unless specified.
9. License
This project is licensed under the MIT License. See the LICENSE file for details (if applicable).
