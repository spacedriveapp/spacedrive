import Foundation
import Combine

class JobListViewModel: ObservableObject {
    @Published var jobs: [JobInfo] = []
    @Published var connectionStatus: ConnectionStatus = .disconnected

    private var daemonConnector: DaemonConnector?
    private var cancellables = Set<AnyCancellable>()

    init() {
        setupDaemonConnector()
    }

    deinit {
        daemonConnector = nil
    }

    private func setupDaemonConnector() {
        daemonConnector = DaemonConnector()

        // Bind daemon connector's published properties to our own
        daemonConnector?.$jobs
            .receive(on: DispatchQueue.main)
            .assign(to: \.jobs, on: self)
            .store(in: &cancellables)

        daemonConnector?.$connectionStatus
            .receive(on: DispatchQueue.main)
            .assign(to: \.connectionStatus, on: self)
            .store(in: &cancellables)
    }

    func reconnect() {
        daemonConnector?.reconnect()
    }

    func disconnect() {
        daemonConnector?.disconnect()
    }

    // MARK: - Computed Properties

    var activeJobs: [JobInfo] {
        jobs.filter { job in
            job.status == .running || job.status == .queued
        }
    }

    var completedJobs: [JobInfo] {
        jobs.filter { job in
            job.status == .completed
        }
    }

    var failedJobs: [JobInfo] {
        jobs.filter { job in
            job.status == .failed
        }
    }

    var jobCounts: (active: Int, completed: Int, failed: Int) {
        return (
            active: activeJobs.count,
            completed: completedJobs.count,
            failed: failedJobs.count
        )
    }

    // MARK: - Helper Methods

    func job(withId id: String) -> JobInfo? {
        return jobs.first { $0.id == id }
    }

    func removeCompletedJobs() {
        jobs.removeAll { job in
            job.status == .completed &&
            job.completedAt != nil &&
            Date().timeIntervalSince(job.completedAt!) > 3600 // Remove completed jobs older than 1 hour
        }
    }

    func clearAllJobs() {
        jobs.removeAll()
    }
}


