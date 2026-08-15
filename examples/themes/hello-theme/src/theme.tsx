import { Button } from "@rtlauncher/theme-ui";
import { defineTheme, type ThemeSlotComponentProps } from "@rtlauncher/theme-sdk";

function HelloAction({ slotId }: ThemeSlotComponentProps) {
  return (
    <Button
      className="hello-theme-action"
      size="sm"
      onClick={() => window.alert(`Hello from ${slotId}`)}
    >
      Hello Theme
    </Button>
  );
}

export default defineTheme({
  id: "com.rtlauncher.example.hello",
  version: "1.0.0",
  apiVersion: "1.0.0",
  setup(context) {
    context.slots.register({
      id: "hello.page-action",
      target: "page.header.actions",
      mode: "after",
      order: 100,
      component: HelloAction,
    });
    return {
      activate() {
        context.events.emit("hello.activated", { version: "1.0.0" });
        context.logger.info("Hello Theme is active.");
      },
    };
  },
});
