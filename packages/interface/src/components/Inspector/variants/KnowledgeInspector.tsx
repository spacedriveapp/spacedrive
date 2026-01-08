import { Paperclip, PaperPlaneRight, Sparkle } from "@phosphor-icons/react";
import clsx from "clsx";
import { useState } from "react";

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
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="border-sidebar-line border-b px-3 py-2.5">
        <div className="flex items-center gap-2">
          <div className="flex size-7 items-center justify-center rounded-full bg-accent/20">
            <Sparkle className="size-4 text-accent" weight="fill" />
          </div>
          <div>
            <div className="font-semibold text-sidebar-ink text-sm">
              AI Assistant
            </div>
            <div className="text-[10px] text-sidebar-inkDull">
              Knowledge & Insights
            </div>
          </div>
        </div>
      </div>

      {/* Messages */}
      <div className="no-scrollbar flex-1 space-y-4 overflow-y-auto px-3 py-4">
        {messages.map((msg) => (
          <div
            className={clsx(
              "flex gap-2.5",
              msg.role === "user" ? "flex-row-reverse" : "flex-row"
            )}
            key={msg.id}
          >
            {/* Avatar */}
            <div
              className={clsx(
                "flex size-7 shrink-0 items-center justify-center rounded-full",
                msg.role === "assistant"
                  ? "bg-accent/20"
                  : "bg-sidebar-selected"
              )}
            >
              {msg.role === "assistant" ? (
                <Sparkle className="size-3.5 text-accent" weight="fill" />
              ) : (
                <div className="font-bold text-[10px] text-sidebar-ink">U</div>
              )}
            </div>

            {/* Message content */}
            <div
              className={clsx(
                "flex max-w-[80%] flex-col",
                msg.role === "user" ? "items-end" : "items-start"
              )}
            >
              <div
                className={clsx(
                  "rounded-lg px-3 py-2",
                  msg.role === "assistant"
                    ? "border border-app-line/50 bg-app-box/60"
                    : "border border-accent/20 bg-accent/10"
                )}
              >
                <p className="text-sidebar-ink text-xs leading-relaxed">
                  {msg.content}
                </p>
              </div>
              <span className="mt-1 px-1 text-[10px] text-sidebar-inkDull">
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
      <div className="space-y-2 border-sidebar-line border-t p-3">
        <div className="flex items-end gap-2">
          <button
            className="rounded-lg p-2 text-sidebar-inkDull transition-colors hover:bg-sidebar-selected hover:text-sidebar-ink"
            title="Attach file"
          >
            <Paperclip className="size-4" weight="bold" />
          </button>

          <div className="flex flex-1 items-center gap-2 rounded-lg border border-app-line bg-app-box px-3 py-2">
            <input
              className="flex-1 bg-transparent text-sidebar-ink text-sm outline-none placeholder:text-sidebar-inkDull"
              onChange={(e) => setMessage(e.target.value)}
              onKeyPress={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleSend();
                }
              }}
              placeholder="Ask me anything..."
              type="text"
              value={message}
            />
          </div>

          <button
            className={clsx(
              "rounded-lg p-2 transition-colors",
              message.trim()
                ? "bg-accent text-white hover:bg-accent/90"
                : "cursor-not-allowed bg-app-box text-sidebar-inkDull"
            )}
            disabled={!message.trim()}
            onClick={handleSend}
            title="Send message"
          >
            <PaperPlaneRight className="size-4" weight="fill" />
          </button>
        </div>

        {/* Quick actions */}
        <div className="flex flex-wrap gap-1.5">
          <button className="rounded-md bg-app-box/40 px-2.5 py-1.5 font-medium text-[11px] text-sidebar-inkDull transition-colors hover:bg-app-box/60 hover:text-sidebar-ink">
            Organize files
          </button>
          <button className="rounded-md bg-app-box/40 px-2.5 py-1.5 font-medium text-[11px] text-sidebar-inkDull transition-colors hover:bg-app-box/60 hover:text-sidebar-ink">
            Find duplicates
          </button>
          <button className="rounded-md bg-app-box/40 px-2.5 py-1.5 font-medium text-[11px] text-sidebar-inkDull transition-colors hover:bg-app-box/60 hover:text-sidebar-ink">
            Smart search
          </button>
        </div>
      </div>
    </div>
  );
}
