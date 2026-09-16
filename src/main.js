const { invoke } = window.__TAURI__.core;
let userText;
let roslinResponse;

const SYSTEM_PROMPT = "You are Roslin, a natural-language file management assistant embedded on the user's Windows computer (Assume the Windows username is \"Kamsi\". This is a placeholder because you are a v1 prototype).\n\nRules you must always follow:\n- For any request follow Windows' file management conventions.\n- For any request that involves deleting files, you must first call stage_deletion (never execute_staged_deletion directly).\n- After staging, summarize exactly which files would be deleted and why, then ask the user to confirm in plain language.\n- Only call execute_staged_deletion if the user's most recent message is a clear, explicit confirmation (e.g. \"yes\", \"confirm\", \"go ahead and delete stage-1\").\n- If a user's instruction is ambiguous (e.g. which folder, which files), ask a clarifying question instead of guessing. \n- For content-based searches (e.g., \"find the file with my phone number\"), use find_files first to get candidates by filename, then read_file on the promising matches. \n- Avoid calling list_files on large directories. Use find_files with specific keywords first to narrow down results. \n- If you need more info about a file, call read_file on it specifically, not list_files on the whole directory.\n- Keep responses short and concrete (do not use emojis): state what you found, what you're about to do, and what you actually did.";

let conversation = [];

async function loadConversation() {
  try {
    conversation = await invoke("load_conversation");
    
    if (conversation.length > 0) {
      const lastMessage = conversation[conversation.length - 1];
      if (lastMessage.role === "assistant" && Array.isArray(lastMessage.content)) {
        const textContent = lastMessage.content.find(c => c.type === "text");
        if (textContent) {
          roslinResponse.textContent = textContent.text;
        }
      }
    } else {
      roslinResponse.textContent = "Hi, I'm Roslin. What would you like me to help you with?";
    }
  } catch (error) {
    console.error("Error loading conversation:", error);
    conversation = [];
    roslinResponse.textContent = "Hi, I'm Roslin. What would you like me to help you with?";
  }
}

async function saveConversation() {
  try {
    await invoke("save_conversation", { conversation });
  } catch (error) {
    console.error("Error saving conversation:", error);
  }
}

async function clearConversation() {
  try {
    await invoke("clear_conversation");
    conversation = []; // Clear in-memory conversation too
    roslinResponse.textContent = "Conversation cleared. What would you like me to help you with?";
  } catch (error) {
    console.error("Error clearing conversation:", error);
  }
}

async function claude_call(userText) {
  conversation.push({ role: "user", content: userText });

  const confirmWords = ["yes", "confirm", "confirmed", "go ahead"];
  const humanConfirmed = confirmWords.some(w => userText.toLowerCase().includes(w));
  
  try {
    roslinResponse.textContent = "Loading...";

    const result = await invoke("claude_call", {
      systemPrompt: SYSTEM_PROMPT,
      messages: JSON.stringify(conversation),
      humanConfirmed: humanConfirmed,
    });

    conversation = result.messages;
    roslinResponse.textContent = result.text;
    
    await saveConversation();
  } catch (error) {
    console.error("Error calling Claude API:", error);
    roslinResponse.textContent = "Error occurred while processing your request.";
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  userText = document.querySelector("#greet-input");
  roslinResponse = document.querySelector("#greet-msg");
  
  await loadConversation();
  
  document.querySelector("#clear-conversation").addEventListener("click", async (e) => {
    e.preventDefault();
    await clearConversation();
  });

  document.querySelector("#greet-form").addEventListener("submit", (e) => {
    e.preventDefault();
    claude_call(userText.value);
    userText.value = "";
  });
});