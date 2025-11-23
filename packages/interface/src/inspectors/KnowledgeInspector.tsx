import { Sparkle, PaperPlaneRight, Paperclip } from "@phosphor-icons/react";
import { useState } from "react";
import clsx from "clsx";

interface Message {
  id: number;
  role: "user" | "assistant";
  content: string;
  timestamp: Date;
}

export function KnowledgeInspector() {
  const [message, setMessage] = useState("");
  const [messages, setMessages] = useState<Message[]>([
    {
      id: 1,
      role: "assistant",
      content:
        "Hi! I'm your AI assistant for Spacedrive. I can help you organize files, search your library, and answer questions about your data.",
      timestamp: new Date(),
    },
  ]);

  const handleSend = () => {
    if (!message.trim()) return;

    const newMessage: Message = {
      id: messages.length + 1,
      role: "user",
      content: message,
      timestamp: new Date(),
    };

    setMessages([...messages, newMessage]);
    setMessage("");

    // Simulate AI response
    setTimeout(() => {
      const aiResponse: Message = {
        id: messages.length + 2,
        role: "assistant",
        content: "This is a prototype. AI responses will be implemented soon!",
        timestamp: new Date(),
      };
      setMessages((prev) => [...prev, aiResponse]);
    }, 500);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="px-3 py-2.5 border-b border-sidebar-line">
        <div className="flex items-center gap-2">
          <div className="size-7 rounded-full bg-accent/20 flex items-center justify-center">
            <Sparkle className="size-4 text-accent" weight="fill" />
          </div>
          <div>
            <div className="text-sm font-semibold text-sidebar-ink">
              AI Assistant
            </div>
            <div className="text-[10px] text-sidebar-inkDull">
              Knowledge & Insights
            </div>
          </div>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-3 py-4 space-y-4 no-scrollbar">
        {messages.map((msg) => (
          <div
            key={msg.id}
            className={clsx(
              "flex gap-2.5",
              msg.role === "user" ? "flex-row-reverse" : "flex-row",
            )}
          >
            {/* Avatar */}
            <div
              className={clsx(
                "size-7 rounded-full shrink-0 flex items-center justify-center",
                msg.role === "assistant"
                  ? "bg-accent/20"
                  : "bg-sidebar-selected",
              )}
            >
              {msg.role === "assistant" ? (
                <Sparkle className="size-3.5 text-accent" weight="fill" />
              ) : (
                <div className="text-[10px] font-bold text-sidebar-ink">U</div>
              )}
            </div>

            {/* Message content */}
            <div
              className={clsx(
                "flex flex-col max-w-[80%]",
                msg.role === "user" ? "items-end" : "items-start",
              )}
            >
              <div
                className={clsx(
                  "px-3 py-2 rounded-lg",
                  msg.role === "assistant"
                    ? "bg-app-box/60 border border-app-line/50"
                    : "bg-accent/10 border border-accent/20",
                )}
              >
                <p className="text-xs text-sidebar-ink leading-relaxed">
                  {msg.content}
                </p>
              </div>
              <span className="text-[10px] text-sidebar-inkDull mt-1 px-1">
                {msg.timestamp.toLocaleTimeString([], {
                  hour: "2-digit",
                  minute: "2-digit",
                })}
              </span>
            </div>
          </div>
        ))}
      </div>

      {/* Input */}
      <div className="border-t border-sidebar-line p-3 space-y-2">
        <div className="flex items-end gap-2">
          <button
            className="p-2 rounded-lg hover:bg-sidebar-selected transition-colors text-sidebar-inkDull hover:text-sidebar-ink"
            title="Attach file"
          >
            <Paperclip className="size-4" weight="bold" />
          </button>

          <div className="flex-1 flex items-center gap-2 bg-app-box border border-app-line rounded-lg px-3 py-2">
            <input
              type="text"
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              onKeyPress={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleSend();
                }
              }}
              placeholder="Ask me anything..."
              className="flex-1 bg-transparent text-sm text-sidebar-ink placeholder:text-sidebar-inkDull outline-none"
            />
          </div>

          <button
            onClick={handleSend}
            disabled={!message.trim()}
            className={clsx(
              "p-2 rounded-lg transition-colors",
              message.trim()
                ? "bg-accent hover:bg-accent/90 text-white"
                : "bg-app-box text-sidebar-inkDull cursor-not-allowed",
            )}
            title="Send message"
          >
            <PaperPlaneRight className="size-4" weight="fill" />
          </button>
        </div>

        {/* Quick actions */}
        <div className="flex flex-wrap gap-1.5">
          <button className="px-2.5 py-1.5 text-[11px] font-medium text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 rounded-md transition-colors">
            Organize files
          </button>
          <button className="px-2.5 py-1.5 text-[11px] font-medium text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 rounded-md transition-colors">
            Find duplicates
          </button>
          <button className="px-2.5 py-1.5 text-[11px] font-medium text-sidebar-inkDull hover:text-sidebar-ink bg-app-box/40 hover:bg-app-box/60 rounded-md transition-colors">
            Smart search
          </button>
        </div>
      </div>
    </div>
  );
}
