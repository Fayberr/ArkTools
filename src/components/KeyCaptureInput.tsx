import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
} from "react";
import {
  captureHotkey,
  formatHotkeyForDisplay,
  getKeyboardLayoutMap,
} from "../hotkeys";

interface Props {
  value: string;
  onChange: (next: string) => void;
  className?: string;
  style?: CSSProperties;
}

// Bare modifier presses can't serve as the auto-press key.
const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta"]);

export default function KeyCaptureInput({
  value,
  onChange,
  className,
  style,
}: Props) {
  const [listening, setListening] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [layoutMap, setLayoutMap] =
    useState<Awaited<ReturnType<typeof getKeyboardLayoutMap>>>(null);

  useEffect(() => {
    let active = true;
    getKeyboardLayoutMap().then((map) => {
      if (active) setLayoutMap(map);
    });
    return () => {
      active = false;
    };
  }, []);

  const displayText = useMemo(() => {
    if (listening) return "Press a key...";
    if (!value) return "Select key";
    return formatHotkeyForDisplay(value, layoutMap);
  }, [layoutMap, listening, value]);

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      setListening(false);
      event.currentTarget.blur();
      return;
    }

    if (
      (event.key === "Backspace" || event.key === "Delete") &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.shiftKey &&
      !event.metaKey
    ) {
      onChange("");
      setListening(false);
      event.currentTarget.blur();
      return;
    }

    // Ignore bare modifier presses.
    if (MODIFIER_KEYS.has(event.key)) return;

    // Capture without modifiers — we only want the main key.
    const captured = captureHotkey({
      key: event.key,
      code: event.code,
      location: event.location,
      ctrlKey: false,
      altKey: false,
      shiftKey: false,
      metaKey: false,
    });

    if (captured) {
      const mainKey = captured.split("+").pop() ?? captured;
      onChange(mainKey);
      setListening(false);
      event.currentTarget.blur();
    }
  };

  const handleContextMenu = (event: MouseEvent<HTMLInputElement>) => {
    event.preventDefault();
    event.stopPropagation();

    onChange("");
    setListening(false);
    inputRef.current?.blur();
  };

  return (
    <input
      ref={inputRef}
      type="text"
      className={className}
      value={displayText}
      readOnly
      onFocus={() => setListening(true)}
      onBlur={() => setListening(false)}
      onKeyDown={handleKeyDown}
      onContextMenu={handleContextMenu}
      spellCheck={false}
      title="Right click input to clear"
      style={{
        cursor: "pointer",
        textAlign: "center",
        ...style,
      }}
    />
  );
}
