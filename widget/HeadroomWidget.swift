// Headroom macOS WidgetKit Suite — Small (170x170), Medium (364x170), Large (364x382)
// Reads WidgetSnapshot.json from App Group container `group.in.4nkitd.headroom`

import AppKit
import WidgetKit
import SwiftUI

struct WidgetAccount: Codable, Identifiable {
    var id: String { provider_id }
    let provider_id: String
    let provider_name: String
    let badge: String
    let badge_bg: UInt32
    let badge_fg: UInt32
    let account_label: String
    let is_primary: Bool
    let percent_left: Float
    let cadence: String
    let resets_at: String?
    let status: String
}

struct WidgetSnapshot: Codable {
    let synced_at: String
    let accounts: [WidgetAccount]
}

struct Provider: TimelineProvider {
    typealias Entry = HeadroomEntry

    func placeholder(in context: Context) -> HeadroomEntry {
        HeadroomEntry(date: Date(), snapshot: sampleSnapshot())
    }

    func getSnapshot(in context: Context, completion: @escaping (HeadroomEntry) -> Void) {
        completion(HeadroomEntry(date: Date(), snapshot: loadSnapshot()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<HeadroomEntry>) -> Void) {
        let entry = HeadroomEntry(date: Date(), snapshot: loadSnapshot())
        let nextUpdate = Calendar.current.date(byAdding: .minute, value: 5, to: Date()) ?? Date()
        let timeline = Timeline(entries: [entry], policy: .after(nextUpdate))
        completion(timeline)
    }

    private func loadSnapshot() -> WidgetSnapshot {
        let fileManager = FileManager.default
        let homeDir = fileManager.homeDirectoryForCurrentUser

        let sandboxURL = homeDir.appendingPathComponent("Library/Containers/com.4nkitd.headroom.widget/Data/WidgetSnapshot.json")
        let appGroupFallback = homeDir.appendingPathComponent("Library/Group Containers/group.in.4nkitd.headroom/WidgetSnapshot.json")

        let containerURL = fileManager.containerURL(forSecurityApplicationGroupIdentifier: "group.in.4nkitd.headroom")?.appendingPathComponent("WidgetSnapshot.json") ?? appGroupFallback

        let candidates = [containerURL, sandboxURL, appGroupFallback]
        for url in candidates {
            if fileManager.fileExists(atPath: url.path),
               let data = try? Data(contentsOf: url),
               let snapshot = try? JSONDecoder().decode(WidgetSnapshot.self, from: data) {
                return snapshot
            }
        }

        return sampleSnapshot()
    }

    private func sampleSnapshot() -> WidgetSnapshot {
        WidgetSnapshot(
            synced_at: "synced 12s ago",
            accounts: [
                WidgetAccount(
                    provider_id: "antigravity:4nkitd@gmail.com",
                    provider_name: "Antigravity",
                    badge: "G",
                    badge_bg: 0x4285f4,
                    badge_fg: 0xffffff,
                    account_label: "4nkit",
                    is_primary: true,
                    percent_left: 77.9,
                    cadence: "Gemini weekly",
                    resets_at: "5d 12h",
                    status: "warn"
                ),
                WidgetAccount(
                    provider_id: "antigravity:3anjuy@gmail.com",
                    provider_name: "Antigravity",
                    badge: "G",
                    badge_bg: 0x4285f4,
                    badge_fg: 0xffffff,
                    account_label: "3anju",
                    is_primary: false,
                    percent_left: 100.0,
                    cadence: "Gemini weekly",
                    resets_at: "6d",
                    status: "ok"
                )
            ]
        )
    }
}

struct HeadroomEntry: TimelineEntry {
    let date: Date
    let snapshot: WidgetSnapshot
}

struct StripedProgressBar: View {
    let fraction: Float
    let color: Color

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 3.5)
                    .fill(Color.white.opacity(0.12))
                    .frame(height: 7)

                RoundedRectangle(cornerRadius: 3.5)
                    .fill(color)
                    .frame(width: max(0, geo.size.width * CGFloat(fraction.clamped(to: 0...1))), height: 7)
            }
        }
        .frame(height: 7)
    }
}

extension Color {
    init(hex: UInt32) {
        let r = Double((hex >> 16) & 0xFF) / 255.0
        let g = Double((hex >> 8) & 0xFF) / 255.0
        let b = Double(hex & 0xFF) / 255.0
        self.init(red: r, green: g, blue: b)
    }
}

extension Comparable {
    func clamped(to limits: ClosedRange<Self>) -> Self {
        return min(max(self, limits.lowerBound), limits.upperBound)
    }
}

struct SmallWidgetView: View {
    let snapshot: WidgetSnapshot

    var mostConstrained: WidgetAccount? {
        snapshot.accounts.min(by: { $0.percent_left < $1.percent_left })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(mostConstrained?.badge ?? "H")
                    .font(.system(size: 10, weight: .bold, design: .monospaced))
                    .padding(4)
                    .background(Color(hex: mostConstrained?.badge_bg ?? 0x2563eb))
                    .foregroundColor(.white)
                    .cornerRadius(4)
                Spacer()
                Text(mostConstrained?.account_label ?? "4nkit")
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .foregroundColor(.secondary)
            }

            Spacer()

            Text("\(Int(mostConstrained?.percent_left.rounded() ?? 78))%")
                .font(.system(size: 32, weight: .bold, design: .monospaced))
                .foregroundColor(mostConstrained?.percent_left ?? 100 < 35 ? .yellow : .green)

            Text(mostConstrained?.cadence ?? "Gemini weekly")
                .font(.system(size: 11, weight: .medium))
                .foregroundColor(.secondary)

            if let reset = mostConstrained?.resets_at {
                Text(reset)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
            }
        }
        .padding()
        .containerBackground(for: .widget) { Color.black }
    }
}

struct MediumWidgetView: View {
    let snapshot: WidgetSnapshot

    var accounts: [WidgetAccount] {
        Array(snapshot.accounts.prefix(2))
    }

    var body: some View {
        HStack(spacing: 16) {
            ForEach(accounts) { acc in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(acc.badge)
                            .font(.system(size: 10, weight: .bold, design: .monospaced))
                            .padding(4)
                            .background(Color(hex: acc.badge_bg))
                            .foregroundColor(.white)
                            .cornerRadius(4)
                        Text(acc.account_label)
                            .font(.system(size: 11, weight: .medium, design: .monospaced))
                            .foregroundColor(.secondary)
                    }

                    Spacer()

                    Text("\(Int(acc.percent_left.rounded()))%")
                        .font(.system(size: 28, weight: .bold, design: .monospaced))
                        .foregroundColor(acc.percent_left < 35 ? .yellow : .green)

                    Text(acc.cadence)
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)

                    if let reset = acc.resets_at {
                        Text(reset)
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundColor(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                if acc.id != accounts.last?.id {
                    Divider()
                }
            }
        }
        .padding()
        .containerBackground(for: .widget) { Color.black }
    }
}

struct LargeWidgetView: View {
    let snapshot: WidgetSnapshot

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("H")
                    .font(.system(size: 10, weight: .bold, design: .monospaced))
                    .padding(4)
                    .background(Color.blue)
                    .foregroundColor(.white)
                    .cornerRadius(4)
                Text("Headroom Quota Board")
                    .font(.system(size: 13, weight: .bold))
                Spacer()
                Text(snapshot.synced_at)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
            }
            .padding(.bottom, 2)

            Divider()

            VStack(spacing: 8) {
                ForEach(snapshot.accounts) { acc in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(acc.account_label)
                                .font(.system(size: 11, weight: .semibold, design: .monospaced))
                            if acc.is_primary {
                                Text("PRIMARY")
                                    .font(.system(size: 8, weight: .bold, design: .monospaced))
                                    .padding(.horizontal, 4)
                                    .padding(.vertical, 1)
                                    .background(Color.green.opacity(0.15))
                                    .foregroundColor(.green)
                                    .cornerRadius(3)
                            }
                            Spacer()
                            Text("\(String(format: "%.1f", acc.percent_left))%")
                                .font(.system(size: 11, weight: .bold, design: .monospaced))
                                .foregroundColor(acc.percent_left < 35 ? .yellow : .green)
                        }

                        HStack {
                            Text(acc.cadence)
                                .font(.system(size: 10))
                                .foregroundColor(.secondary)
                            Spacer()
                            if let reset = acc.resets_at {
                                Text("resets in \(reset)")
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundColor(.secondary)
                            }
                        }

                        StripedProgressBar(
                            fraction: acc.percent_left / 100.0,
                            color: acc.percent_left < 35 ? .yellow : .green
                        )
                    }
                    .padding(8)
                    .background(Color.white.opacity(0.04))
                    .cornerRadius(8)
                }
            }

            Spacer()

            HStack {
                Text("\(snapshot.accounts.count) Configured Accounts")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
                Spacer()
                Text("WidgetKit v2")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
            }
        }
        .padding()
        .containerBackground(for: .widget) { Color.black }
    }
}

@main
struct HeadroomWidgetBundle: WidgetBundle {
    var body: some Widget {
        HeadroomSmallWidget()
        HeadroomMediumWidget()
        HeadroomLargeWidget()
    }
}

struct HeadroomSmallWidget: Widget {
    let kind: String = "HeadroomSmallWidget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: Provider()) { entry in
            SmallWidgetView(snapshot: entry.snapshot)
        }
        .configurationDisplayName("Headroom Quota Gauge")
        .description("Most-constrained subscription headroom gauge.")
        .supportedFamilies([.systemSmall])
    }
}

struct HeadroomMediumWidget: Widget {
    let kind: String = "HeadroomMediumWidget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: Provider()) { entry in
            MediumWidgetView(snapshot: entry.snapshot)
        }
        .configurationDisplayName("Headroom Dual View")
        .description("Compare subscription quotas across accounts.")
        .supportedFamilies([.systemMedium])
    }
}

struct HeadroomLargeWidget: Widget {
    let kind: String = "HeadroomLargeWidget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: Provider()) { entry in
            LargeWidgetView(snapshot: entry.snapshot)
        }
        .configurationDisplayName("Headroom Quota Board")
        .description("All active AI subscription accounts side-by-side.")
        .supportedFamilies([.systemLarge])
    }
}
