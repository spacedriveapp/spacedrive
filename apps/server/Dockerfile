FROM debian:stable-slim

# Arguments and labels
ARG USER=spaceboy
LABEL org.opencontainers.image.title="Spacedrive Server"
LABEL org.opencontainers.image.source="https://github.com/spacedriveapp/spacedrive"

# Install dependencies
RUN apt-get update && apt-get install -y libavdevice-dev libavfilter-dev libavformat-dev libavcodec-dev libavutil-dev

# Copy the compiled server CLI into the container
COPY ./server /sdserver

# Expose webserver
EXPOSE 8080

# Create the data directory to store the database
RUN mkdir /data
ENV DATA_DIR /data

# Drop privledges to non-root user
RUN groupadd -g 1001 $USER && \
    adduser --system --no-create-home --shell /usr/sbin/nologin --uid 1001 --gid 1001 $USER && \
    chown -R $USER /data && \
    chmod -R 770 /data
USER $USER

# Run the CLI when the container is started
ENTRYPOINT [ "/sdserver" ]