#pragma once

#include "room.hpp"

#include <boost/uuid.hpp>
#include <chrono>
#include <shared_mutex>
#include <map>
#include <string>
#include <vector>
#include <functional>
#include <memory>

class room_list_t {
public:
  using logger = std::function<void(const std::string &)>;

  const logger log_error, log_info;

  const std::chrono::minutes lobby_lifetime;
  const std::chrono::minutes game_lifetime;

  [[nodiscard]] explicit room_list_t(
    size_t limit, std::chrono::minutes lobby_lifetime, std::chrono::minutes game_lifetime,
    logger log_error = [](const std::string &) {}, logger log_info = [](const std::string &) {}
  );

  [[nodiscard]] std::shared_ptr<room_t> create(const std::string &version, const room_t::user_t &owner, size_t size);

  [[nodiscard]] std::shared_ptr<room_t> get(boost::uuids::uuid id) const;

  [[nodiscard]] bool exists(boost::uuids::uuid id) const;

  bool remove(boost::uuids::uuid id);

  [[nodiscard]] size_t count() const;

  [[nodiscard]] std::vector<std::shared_ptr<room_t>> get_all() const;

  [[nodiscard]] size_t get_limit() const;

  void set_limit(size_t new_limit);

  void clean(std::chrono::milliseconds user_timeout);

protected:
  mutable std::shared_mutex rooms_mutex;
  std::map<boost::uuids::uuid, std::shared_ptr<room_t>> rooms;
  size_t limit;
};
