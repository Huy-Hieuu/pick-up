import React from "react";
import {
  ActivityIndicator,
  KeyboardAvoidingView,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
  useWindowDimensions,
  type StyleProp,
  type TextInputProps,
  type TextProps,
  type TextStyle,
  type ViewStyle,
} from "react-native";
import Feather from "@expo/vector-icons/Feather";
import { router, type Href } from "expo-router";
import { authReturnRoute } from "../utils/navigation";
import { userFacingError } from "../utils/errors";

export const palette = {
  ink: "#173C32",
  forest: "#193E32",
  green: "#275B47",
  lime: "#D7EE83",
  paper: "#F6F7F2",
  white: "#FFFFFF",
  muted: "#66746B",
  line: "#E0E5DD",
  pale: "#EAF0E4",
  danger: "#AB302D",
  amber: "#865C16",
};
export const type = StyleSheet.create({
  title: {
    fontFamily: "Inter_700Bold",
    fontSize: 34,
    lineHeight: 42,
    letterSpacing: -1.2,
  },
  heading: {
    fontFamily: "Inter_600SemiBold",
    fontSize: 22,
    lineHeight: 30,
    letterSpacing: -0.6,
  },
  subheading: { fontFamily: "Inter_600SemiBold", fontSize: 17, lineHeight: 25 },
  body: { fontFamily: "Inter_400Regular", fontSize: 14, lineHeight: 22 },
  small: { fontFamily: "Inter_400Regular", fontSize: 12, lineHeight: 19 },
  label: {
    fontFamily: "Inter_600SemiBold",
    fontSize: 12,
    lineHeight: 18,
    letterSpacing: 1.1,
  },
});
export function AppText({ style, ...props }: TextProps) {
  return <Text {...props} style={[type.body, { color: palette.ink }, style]} />;
}
export function Icon({
  name,
  size = 20,
  color = palette.ink,
}: {
  name: React.ComponentProps<typeof Feather>["name"];
  size?: number;
  color?: string;
}) {
  return <Feather name={name} size={size} color={color} />;
}
export function Button({
  title,
  onPress,
  loading,
  disabled,
  secondary,
  danger,
  icon,
  style,
}: {
  title: string;
  onPress: () => void;
  loading?: boolean;
  disabled?: boolean;
  secondary?: boolean;
  danger?: boolean;
  icon?: React.ComponentProps<typeof Feather>["name"];
  style?: StyleProp<ViewStyle>;
}) {
  const unavailable = disabled || loading;
  const foreground = danger
    ? palette.danger
    : secondary
      ? palette.ink
      : palette.white;
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={title}
      accessibilityState={{ disabled: !!unavailable, busy: !!loading }}
      disabled={unavailable}
      onPress={onPress}
      style={({ pressed }) => [
        ui.button,
        {
          backgroundColor: secondary || danger ? palette.white : palette.forest,
          borderColor: danger
            ? "#E6C7C5"
            : secondary
              ? palette.line
              : palette.forest,
          opacity: unavailable ? 0.5 : pressed ? 0.8 : 1,
        },
        style,
      ]}
    >
      {loading ? (
        <ActivityIndicator color={foreground} />
      ) : icon ? (
        <Icon name={icon} color={foreground} size={18} />
      ) : null}
      <AppText
        style={{
          color: foreground,
          fontFamily: "Inter_600SemiBold",
          textAlign: "center",
          flexShrink: 1,
        }}
      >
        {title}
      </AppText>
    </Pressable>
  );
}
export function IconButton({
  name,
  label,
  onPress,
  active,
}: {
  name: React.ComponentProps<typeof Feather>["name"];
  label: string;
  onPress: () => void;
  active?: boolean;
}) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityState={{ selected: !!active }}
      onPress={onPress}
      style={ui.iconButton}
    >
      <Icon name={name} color={active ? palette.green : palette.ink} />
    </Pressable>
  );
}
export function Pill({
  title,
  selected,
  onPress,
  icon,
}: {
  title: string;
  selected?: boolean;
  onPress?: () => void;
  icon?: React.ComponentProps<typeof Feather>["name"];
}) {
  return (
    <Pressable
      accessibilityRole={onPress ? "button" : undefined}
      accessibilityState={{ selected: !!selected }}
      onPress={onPress}
      disabled={!onPress}
      style={[
        ui.pill,
        selected && {
          backgroundColor: palette.forest,
          borderColor: palette.forest,
        },
      ]}
    >
      {icon && (
        <Icon
          name={icon}
          size={15}
          color={selected ? palette.white : palette.ink}
        />
      )}
      <AppText
        style={{
          fontSize: 13,
          fontFamily: "Inter_500Medium",
          color: selected ? palette.white : palette.ink,
        }}
      >
        {title}
      </AppText>
    </Pressable>
  );
}
export function Field({
  label,
  error,
  style,
  ...props
}: TextInputProps & { label: string; error?: string }) {
  return (
    <View style={{ gap: 7 }}>
      <AppText style={{ fontFamily: "Inter_500Medium" }}>{label}</AppText>
      <TextInput
        accessibilityLabel={label}
        placeholderTextColor={palette.muted}
        {...props}
        style={[ui.input, error && { borderColor: palette.danger }, style]}
      />
      {error && <Message error>{error}</Message>}
    </View>
  );
}
export function Message({
  children,
  error,
}: {
  children: React.ReactNode;
  error?: boolean;
}) {
  return (
    <View
      accessibilityRole="alert"
      accessibilityLiveRegion="polite"
      style={[
        ui.message,
        { backgroundColor: error ? "#FAECEA" : palette.pale },
      ]}
    >
      <AppText style={{ color: error ? palette.danger : palette.ink }}>
        {children}
      </AppText>
    </View>
  );
}
export function Card({
  children,
  style,
}: {
  children: React.ReactNode;
  style?: StyleProp<ViewStyle>;
}) {
  return <View style={[ui.card, style]}>{children}</View>;
}
export function Page({
  children,
  narrow,
}: {
  children: React.ReactNode;
  narrow?: boolean;
}) {
  const { width } = useWindowDimensions();
  return (
    <KeyboardAvoidingView
      style={{ flex: 1 }}
      behavior={Platform.OS === "ios" ? "padding" : undefined}
    >
      <ScrollView
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={{ flexGrow: 1 }}
      >
        <View
          style={[
            ui.page,
            {
              maxWidth: narrow ? 760 : 1240,
              paddingHorizontal: width < 600 ? 20 : 40,
              paddingVertical: width < 600 ? 24 : 38,
            },
          ]}
        >
          {children}
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
export function SectionTitle({
  title,
  subtitle,
  action,
  onAction,
}: {
  title: string;
  subtitle?: string;
  action?: string;
  onAction?: () => void;
}) {
  return (
    <View
      style={[
        ui.row,
        {
          justifyContent: "space-between",
          marginBottom: 20,
          flexWrap: "wrap",
          gap: 8,
        },
      ]}
    >
      <View style={{ gap: 4, flexShrink: 1 }}>
        <AppText style={type.heading}>{title}</AppText>
        {subtitle && (
          <AppText style={{ color: palette.muted }}>{subtitle}</AppText>
        )}
      </View>
      {action && onAction && (
        <Pressable
          onPress={onAction}
          accessibilityRole="button"
          accessibilityLabel={action}
          style={{ minHeight: 48, justifyContent: "center" }}
        >
          <AppText style={{ fontFamily: "Inter_600SemiBold" }}>
            {action} ↗
          </AppText>
        </Pressable>
      )}
    </View>
  );
}
export function StateBlock({
  loading,
  error,
  empty,
  retry,
  action,
}: {
  loading?: boolean;
  error?: Error | null;
  empty?: string;
  retry?: () => void;
  action?: { title: string; onPress: () => void };
}) {
  if (loading)
    return (
      <View
        accessibilityLiveRegion="polite"
        style={ui.state}
      >
        <ActivityIndicator color={palette.green} />
        <AppText style={{ color: palette.muted }}>Đang tải…</AppText>
      </View>
    );
  if (error)
    return (
      <Card style={ui.state}>
        <Icon name="wifi-off" size={28} />
        <AppText accessibilityRole="header" style={type.subheading}>
          Chưa tải được dữ liệu
        </AppText>
        <AppText accessibilityRole="alert" style={{ textAlign: "center", color: palette.muted }}>
          {userFacingError(error)}
        </AppText>
        {retry && <Button title="Thử lại" secondary onPress={retry} />}
      </Card>
    );
  return (
    <Card style={ui.state}>
      <Icon name="sun" size={32} />
      <AppText accessibilityLiveRegion="polite" style={{ textAlign: "center", color: palette.muted }}>
        {empty || "Chưa có dữ liệu."}
      </AppText>
      {action && <Button title={action.title} secondary onPress={action.onPress} />}
    </Card>
  );
}
export function AuthGate({
  children,
  next,
}: {
  children: React.ReactNode;
  next: string;
}) {
  return (
    <Card style={ui.state}>
      <Icon name="users" size={32} />
      <AppText style={type.heading}>Ra sân cùng cộng đồng</AppText>
      <AppText style={{ color: palette.muted, textAlign: "center" }}>
        {children}
      </AppText>
      <Button
        title="Đăng nhập để tiếp tục"
        onPress={() => router.push({ pathname: "/login", params: { next } })}
      />
      <Pressable
        accessibilityRole="button"
        onPress={() => router.push({ pathname: "/register", params: { next } })}
        style={{ padding: 10, minHeight: 48, minWidth: 48, justifyContent: "center" }}
      >
        <AppText>Tạo tài khoản mới ↗</AppText>
      </Pressable>
    </Card>
  );
}
export function BackLink({ title = "Quay lại" }: { title?: string }) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={title}
      onPress={() => (router.canGoBack() ? router.back() : router.replace("/"))}
      style={[
        ui.row,
        { minHeight: 48, alignSelf: "flex-start", marginBottom: 12 },
      ]}
    >
      <Icon name="arrow-left" size={18} />
      <AppText style={{ color: palette.muted }}>{title}</AppText>
    </Pressable>
  );
}
export function go(path: Href) {
  router.push(path);
}
export function safeReturn(value: string | string[] | undefined): Href {
  return authReturnRoute(value) as Href;
}
export const ui = StyleSheet.create({
  page: { width: "100%", alignSelf: "center", gap: 28 },
  row: { flexDirection: "row", alignItems: "center", gap: 10 },
  button: {
    minHeight: 50,
    paddingHorizontal: 22,
    paddingVertical: 13,
    borderRadius: 12,
    borderWidth: 1,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: 10,
  },
  iconButton: {
    height: 48,
    width: 48,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 24,
  },
  pill: {
    minHeight: 48,
    paddingHorizontal: 17,
    paddingVertical: 11,
    borderWidth: 1,
    borderColor: palette.line,
    backgroundColor: palette.white,
    borderRadius: 24,
    flexDirection: "row",
    alignItems: "center",
    gap: 7,
  },
  input: {
    minHeight: 52,
    borderWidth: 1,
    borderColor: palette.line,
    borderRadius: 12,
    paddingHorizontal: 15,
    paddingVertical: 12,
    backgroundColor: palette.white,
    color: palette.ink,
    fontFamily: "Inter_400Regular",
    fontSize: 15,
  },
  card: {
    backgroundColor: palette.white,
    borderWidth: 1,
    borderColor: palette.line,
    borderRadius: 20,
    padding: 22,
    gap: 15,
  },
  message: { borderRadius: 12, padding: 15 },
  state: {
    alignItems: "center",
    justifyContent: "center",
    paddingVertical: 38,
    paddingHorizontal: 22,
    gap: 15,
  },
  divider: { height: 1, backgroundColor: palette.line },
  eyebrow: {
    fontFamily: "Inter_600SemiBold",
    fontSize: 11,
    letterSpacing: 1.6,
    color: palette.muted,
  },
  link: { minHeight: 48, minWidth: 48, justifyContent: "center" },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: 20 },
});
