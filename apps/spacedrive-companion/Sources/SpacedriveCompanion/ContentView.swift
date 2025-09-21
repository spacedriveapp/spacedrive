import SwiftUI

struct ContentView: View {
    @StateObject private var viewModel = JobListViewModel()

    var body: some View {
        JobMonitorView(viewModel: viewModel)
            .frame(minWidth: 300, minHeight: 400)
            .background(VisualEffectBackground())
            .onAppear {
                // The view model automatically connects when initialized
            }
            .onDisappear {
                viewModel.disconnect()
            }
    }
}

#Preview {
    ContentView()
        .frame(width: 400, height: 600)
}


