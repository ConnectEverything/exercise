FROM scratch
ADD server2.conf /
RUN ./nats-server -addr 10.20.20.2 -c ./server2.conf -js -sd storage
