# rustai
<img src="https://github.com/xi9d/rustai/blob/main/assets/version%201.2.png">
Overview<br>
TouristXi9d is a Rust-based desktop application that serves as an AI client for interacting with the Ollama API. It features Retrieval-Augmented Generation (RAG) for context-aware responses and analytics to track usage metrics. The application uses the egui library for a modern graphical user interface, allowing users to input prompts, load text files for context, view AI-generated responses, and monitor analytics like request counts and response times. Conversations are saved in a SQLite database and as text files for persistence.<br>
Features<br>

Send prompts to an Ollama API server and display responses.<br>
Load text files to provide additional context for AI prompts.<br>
RAG system to retrieve similar past conversations for improved responses.<br>
Analytics dashboard showing total requests, average response time, and more.<br>
Save responses to text files and open the save directory.<br>
Modern, dark-themed UI with a clean and intuitive layout.<br>

Prerequisites<br>
To run the project, ensure you have the following installed:<br>

Rust (latest stable version recommended).<br>
Cargo (comes with Rust).<br>
An Ollama server running locally or accessible at http://localhost:11434/api/generate.<br>
A compatible operating system (Windows, macOS, or Linux).<br>

Getting Started<br>
1. Clone the Repository<br>
Clone the project to your local machine:<br>
git clone <repository-url><br>
cd tourist_xi9d<br>

Replace <repository-url> with the actual repository URL if applicable.<br>
2. Install Dependencies<br>
The project uses several Rust crates (eframe, reqwest, serde, chrono, rusqlite, tokio, rfd). Install them by running:<br>
cargo build --release<br>

This command fetches all dependencies listed in Cargo.toml and builds the project in release mode.<br>
3. Set Up Ollama<br>
Ensure an Ollama server is running and accessible. By default, the application connects to http://localhost:11434/api/generate. To start an Ollama server:<br>

Install Ollama following the instructions at https://ollama.ai/.<br>
Run the server with:<br>

ollama serve<br>

Ensure the model deepseek-r1:7b (or another compatible model) is available. Pull it if needed:<br>

ollama pull deepseek-r1:7b<br>

4. Run the Application<br>
Compile and run the project in release mode:<br>
cargo run --release<br>

This will launch the GUI application, displaying the TouristXi9d interface.<br>
5. Using the Application<br>

Model and URL: Configure the AI model (default: deepseek-r1:7b) and Ollama URL (default: http://localhost:11434/api/generate) in the configuration section.<br>
Load File: Click "Load Text File" to upload a text file for additional context.<br>
Input Prompt: Enter your prompt in the input text area and click "Generate Response" to get an AI response.<br>
Analytics: View usage metrics in the right panel (toggle with "Show Analytics").<br>
RAG: Enable RAG to see similar past prompts and responses (toggle with "Enable RAG").<br>
Save and Copy: Copy or save the output to a file, or open the save directory (./tourist_data) to view saved responses.<br>

6. Project Structure<br>

src/main.rs: Contains the main application logic, UI, and RAG system implementation.<br>
tourist_data/: Directory where conversation data is saved as text files and a SQLite database (conversations.db).<br>
Cargo.toml: Lists project dependencies and configuration.<br>

7. Troubleshooting<br>

Ollama Connection Issues: Ensure the Ollama server is running and the URL is correct. Check firewall settings if connecting to a remote server.<br>
Build Errors: Verify Rust is up to date (rustup update) and all dependencies are installed.<br>
UI Issues: Ensure your system supports OpenGL (required by eframe).

8. Contributing
Feel free to submit issues or pull requests to improve the project. Ensure any changes maintain the existing functionality and focus on enhancing the UI or performance without adding new features unless specified.
9. License
This project is licensed under the MIT License. See the LICENSE file for details (if applicable).
